//File for sensor readings

pub struct Threshold {
    min_value: f32,
    max_value: f32,
}

impl Threshold {
    fn new(min_value: f32, max_value:f32) -> Self{
        Threshold {
            min_value,
            max_value,
        }
    }
    fn within_range(&self, value: f32) -> bool {
        value >= self.min_value && value <= self.max_value
    }
}

pub fn thresh_test(column: String, value: f32) -> u32 {
    let threshold = match column.as_str() {
        "field" => {Threshold::new(0.0, 30000.0)}
        "status" => {Threshold::new(0.0, 5.0)}
        "signal" => {Threshold::new(0.0, 1.0)}
        "signal.detector" => {Threshold::new(0.0, 7.4)}
        "laser.therm.control.error" => {Threshold::new(0.0, 0.7)}
        "laser.therm.control.monitor" => {Threshold::new(0.0, -0.5)}
        "laser.therm.heater.power" => {Threshold::new(0.0, 2.0)}
        "cell.therm.sensor.measure" => {Threshold::new(0.0, 80.0)}
        "cell.therm.heater.power"=> {Threshold::new(0.0, 2.0)}
        "vco.error" => {Threshold::new(0.0, 2.0)}
        "vco.pull" => {Threshold::new(0.0, 2.0)}
        "mcu.therm" => {Threshold::new(0.0, 35.0)}
        _ => {
            return 0;
        }
    };
    
    if threshold.within_range(value) {
        return 2; //green
        
    } else{
        return 3; //red
        
    }
}