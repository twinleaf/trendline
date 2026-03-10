use crate::shared::UiRpcError;
use crossbeam::channel::{bounded, unbounded, Receiver, Sender};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use twinleaf::device::{util as tl_util, RpcClient, RpcDataKind, RpcMeta, RpcRegistry, RpcValue};
use twinleaf::tio::proto::DeviceRoute;
use twinleaf::tio::proxy;

pub enum RpcMsg {
    SetProxy {
        proxy: Arc<proxy::Interface>,
    },
    LoadSpecs {
        route: DeviceRoute,
        reply: Sender<Result<Vec<RpcMeta>, UiRpcError>>,
    },
    Execute {
        route: DeviceRoute,
        name: String,
        args: Value,
        reply: Sender<Result<Value, UiRpcError>>,
    },
    Shutdown,
}

struct RpcModel {
    proxy: Option<Arc<proxy::Interface>>,
    rpc_client: Option<RpcClient>,
    registries: HashMap<DeviceRoute, RpcRegistry>,
}

pub struct RpcManager {
    tx: Sender<RpcMsg>,
}

impl RpcManager {
    pub fn new() -> Self {
        let (tx, rx) = unbounded::<RpcMsg>();
        Self::spawn_actor(rx);
        Self { tx }
    }

    fn spawn_actor(rx: Receiver<RpcMsg>) {
        thread::Builder::new()
            .name("rpc-manager".into())
            .spawn(move || {
                let mut model = RpcModel {
                    proxy: None,
                    rpc_client: None,
                    registries: HashMap::new(),
                };

                while let Ok(msg) = rx.recv() {
                    if !Self::update(msg, &mut model) {
                        break;
                    }
                }
            })
            .expect("Failed to spawn rpc-manager thread.");
    }

    fn update(msg: RpcMsg, model: &mut RpcModel) -> bool {
        match msg {
            RpcMsg::SetProxy { proxy } => {
                model.proxy = Some(proxy);
                model.rpc_client = None;
                model.registries.clear();
            }
            RpcMsg::LoadSpecs { route, reply } => {
                let result = Self::handle_load_specs(model, &route);
                let _ = reply.send(result);
            }
            RpcMsg::Execute {
                route,
                name,
                args,
                reply,
            } => {
                let result = Self::handle_execute(model, &route, &name, args);
                let _ = reply.send(result);
            }
            RpcMsg::Shutdown => return false,
        }
        true
    }

    fn handle_load_specs(
        model: &mut RpcModel,
        route: &DeviceRoute,
    ) -> Result<Vec<RpcMeta>, UiRpcError> {
        let client = Self::ensure_client(model)?;

        let nrpcs: u16 = client.get(route, "rpc.listinfo")?;
        let mut specs = Vec::with_capacity(nrpcs as usize);
        for id in 0..nrpcs {
            let (meta, name): (u16, String) = client.rpc(route, "rpc.listinfo", id)?;
            specs.push(tl_util::parse_rpc_spec(meta, name));
        }

        let registry = RpcRegistry::new(specs.clone());
        model.registries.insert(route.clone(), registry);

        Ok(specs)
    }

    fn handle_execute(
        model: &mut RpcModel,
        route: &DeviceRoute,
        name: &str,
        args: Value,
    ) -> Result<Value, UiRpcError> {
        Self::ensure_client(model)?;

        let registry = model.registries.get(route).ok_or_else(|| {
            UiRpcError::AppLogic(format!("No RPC registry loaded for route '{}'.", route))
        })?;

        let meta = registry.find(name).ok_or_else(|| {
            UiRpcError::AppLogic(format!("RPC '{}' not found for route '{}'.", name, route))
        })?;

        let arg_bytes = value_to_rpc_bytes(&args, &meta.data_kind)?;
        let client = model
            .rpc_client
            .as_ref()
            .ok_or_else(|| UiRpcError::AppLogic("RPC client is not initialized.".to_string()))?;
        let reply_bytes = client.raw_rpc(route, name, &arg_bytes)?;
        let rpc_value = tl_util::rpc_decode_reply(&reply_bytes, &meta.data_kind)
            .map_err(|e| UiRpcError::AppLogic(format!("Failed to decode RPC reply: {:?}", e)))?;

        Ok(rpc_value_to_json(rpc_value))
    }

    fn ensure_client(model: &mut RpcModel) -> Result<&RpcClient, UiRpcError> {
        if model.rpc_client.is_none() {
            let proxy = model
                .proxy
                .as_ref()
                .ok_or_else(|| UiRpcError::AppLogic("RPC proxy is not initialized.".to_string()))?;
            let client = RpcClient::open(proxy, DeviceRoute::root())
                .map_err(|e| UiRpcError::AppLogic(format!("Failed to open RPC client: {:?}", e)))?;
            model.rpc_client = Some(client);
        }

        model
            .rpc_client
            .as_ref()
            .ok_or_else(|| UiRpcError::AppLogic("RPC client is not initialized.".to_string()))
    }

    pub fn dispatch(&self, msg: RpcMsg) -> Result<(), String> {
        self.tx
            .send(msg)
            .map_err(|_| "RpcManager actor is unavailable.".to_string())
    }

    pub fn ask<R>(&self, build: impl FnOnce(Sender<R>) -> RpcMsg) -> Result<R, String>
    where
        R: Send + 'static,
    {
        let (reply_tx, reply_rx) = bounded(1);
        self.tx
            .send(build(reply_tx))
            .map_err(|_| "RpcManager actor is unavailable.".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "RpcManager actor did not respond.".to_string())
    }
}

fn value_to_rpc_bytes(args: &Value, kind: &RpcDataKind) -> Result<Vec<u8>, String> {
    if args.is_null() {
        return Ok(Vec::new());
    }
    match kind {
        RpcDataKind::Unit => Ok(Vec::new()),
        RpcDataKind::Int {
            signed: false,
            size,
        } => {
            let n = args
                .as_u64()
                .or_else(|| args.as_f64().map(|f| f as u64))
                .ok_or("Expected unsigned integer")?;
            match size {
                1 => Ok((n as u8).to_le_bytes().to_vec()),
                2 => Ok((n as u16).to_le_bytes().to_vec()),
                4 => Ok((n as u32).to_le_bytes().to_vec()),
                8 => Ok(n.to_le_bytes().to_vec()),
                s => Err(format!("Unsupported unsigned int size: {}", s)),
            }
        }
        RpcDataKind::Int { signed: true, size } => {
            let n = args
                .as_i64()
                .or_else(|| args.as_f64().map(|f| f as i64))
                .ok_or("Expected signed integer")?;
            match size {
                1 => Ok((n as i8).to_le_bytes().to_vec()),
                2 => Ok((n as i16).to_le_bytes().to_vec()),
                4 => Ok((n as i32).to_le_bytes().to_vec()),
                8 => Ok(n.to_le_bytes().to_vec()),
                s => Err(format!("Unsupported signed int size: {}", s)),
            }
        }
        RpcDataKind::Float { size } => {
            let f = args
                .as_f64()
                .or_else(|| args.as_i64().map(|n| n as f64))
                .or_else(|| args.as_u64().map(|n| n as f64))
                .ok_or("Expected number")?;
            match size {
                4 => Ok((f as f32).to_le_bytes().to_vec()),
                8 => Ok(f.to_le_bytes().to_vec()),
                s => Err(format!("Unsupported float size: {}", s)),
            }
        }
        RpcDataKind::String { max_len } => {
            let s = args.as_str().ok_or("Expected string")?;
            if let Some(max) = max_len {
                if s.len() > *max as usize {
                    return Err(format!("String too long: {} > {}", s.len(), max));
                }
            }
            Ok(s.as_bytes().to_vec())
        }
        RpcDataKind::Raw { .. } => Ok(Vec::new()),
    }
}

fn rpc_value_to_json(value: RpcValue) -> Value {
    match value {
        RpcValue::Unit => Value::Null,
        RpcValue::U64(n) => json!(n),
        RpcValue::I64(n) => json!(n),
        RpcValue::F64(f) => json!(f),
        RpcValue::Str(s) => json!(s),
        RpcValue::Bytes(b) => json!(b),
    }
}
