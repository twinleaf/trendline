# Lily

## Name
VMR Sensor Graphing Tool

## Description
Provides basic graphing of stream data via resizable uPlot graphs. The project uses a Rust backend and Vanilla Javascript on the Tauri frontend. The cli tool can be used within conjunction of the connect tool to run a live stream monitor of the incoming sensor data.

## Installation
To run the GUI, you can run two options in the terminal 

Within src-tauri:
`cargo run --bin lily`

Or within target debug run:
`./target/debug/lily`

Below is an example of what the GUI should look like
![Alt text](Lily/src-tauri/icons/demo.png)



To use the live stream monitor: (**in progress tool)

1) Connect to the sensor 
`cargo run --bin connect`

2) In a new terminal run the below command with "stream" or "arm" (armstrong) depending on sensor:
`cargo run --bin cli {}` 


## License
For open source projects, say how it is licensed.

## Project status
Development in progress