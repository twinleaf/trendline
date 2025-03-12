const { Webview } = window.__TAURI__.webview;
const { getCurrentWebviewWindow } = window.__TAURI__.webviewWindow;
const { once } = window.__TAURI__.event;
const { invoke } = window.__TAURI__.core;

invoke('serial_ports')
const webpage = getCurrentWebviewWindow();
var port_names = [];
let names_loaded = false;

webpage.once("ports", (event) => {
    let names = event.payload;
    port_names = names;
    names_loaded = true;
    
});


new Promise((resolve) => {
    const checkLoad = setInterval(() => {
        if (names_loaded) {
            clearInterval(checkLoad);
            resolve();
        }
    }, 100);
}).then(() => {
    setTimeout(() => {
        const connectbox = document.getElementById('ports');
        
        port_names.forEach(name => {
            const port = document.createElement('button')
            port.innerText = "Connect";
            port.className = "serials";
            port.id = name;

            const label = document.createElement('label');
            label.innerText = name;
            label.htmlFor = port.id;
            
            connectbox.appendChild(label);
            connectbox.appendChild(port);
            connectbox.appendChild(document.createElement('br'))
        }) 
    
        document.querySelectorAll('.serials').forEach(serial => {
            serial.addEventListener("click", function() {
                webpage.emit('connect', serial.id)
            })
        })

        document.getElementById('typeButton').addEventListener("click", function(){
            let enteredValue = document.getElementById('textfield')
            webpage.emit('connect', enteredValue.value)
        })

        document.getElementById('textfield').addEventListener("keypress", function(e) {
            if (e.key == "Enter") {webpage.emit('connect', document.getElementById('textfield').value)}

        })
    }, 100)
})



