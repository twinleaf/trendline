const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { Window } = window.__TAURI__.window;
const { Webview } = window.__TAURI__.webview;
const { getCurrentWebviewWindow } = window.__TAURI__.webviewWindow;
const { once } = window.__TAURI__.event;

invoke('graph_data');

webpage = getCurrentWebviewWindow();
window.onload = () => {
    var graphs = []; 
    var columns = []; 
    var rpcs = [];
    var serial = []; 
    let timePoints = 10;

    let startTime = Date.now();
    gotNames = false;
    webpage.listen("main", (event) => {
        const [values, name, header] = event.payload;
        const elapsed = (Date.now() - startTime) /1000;
        
        if (!gotNames) {
            for (let i = 0; i< name.length; i++) {columns.push(name[i])}
            serial.push(header)
            gotNames = true;
        } 

        //push data to each graph
        graphs.forEach((chart, index) => {  
            for (let i = 0; i < values[index].length; i++) {  
                chart.data[0].push(elapsed)
                chart.data[1].push(values[index][i])

                let firstLogTime = chart.data[0][0]
                let recentLogTime = chart.data[0][chart.data[0].length -1] //last timestamp

                if ((recentLogTime - firstLogTime) > timePoints){
                    chart.data[0].shift();
                    chart.data[1].shift();
                }
            }   
            
            const timeSpan = document.getElementById('timeSpan');
            timeSpan.addEventListener('keypress', function(e) {
                if (e.key == "Enter") {
                    timePoints = in_range(timeSpan);
                    while ((recentLogTime - firstLogTime)> timePoints) {
                        chart.data[0].shift();
                        chart.data[1].shift();
                        firstLogTime = chart.data[0][0]
                        recentLogTime = chart.data[0][chart.data[0].length -1]
                    } 
                    timeSpan.innerHTML = timePoints;
                    timeSpan.value = timePoints;
                }
            }) 
            chart.setData(chart.data, true);
        }) 
    });

    webpage.once("rpcs", (event) => {
        const controls = event.payload;
        for (let i = 0; i< controls.length; i++) {rpcs.push(controls[i])}
    })

    //on button click return rpc
    const buttonChange = document.querySelectorAll('.buttonCommands');
    buttonChange.forEach(clickButton => {
        clickButton.addEventListener("click", function() {   
            call = [clickButton.id, clickButton.value];     
            webpage.emit('returningRPCName', call);      
        })
    })

    const inputChange = document.querySelectorAll('.InputCommands');
    const toggleChange = document.querySelectorAll('.checkCommands') 
    //on input change call rpc command
    inputChange.forEach(rpccall => {
        rpccall.addEventListener('keypress', function(e) {
            value = in_range(rpccall).toString()
            if (e.key == "Enter") {
                call= [rpccall.id, value];
                webpage.emit('returningRPCName', call);    
            }
        })
    })

    //on checkbox change call rpc command
    toggleChange.forEach(clickToggle => {
        clickToggle.addEventListener("change", (event)=>{
            let num = 0;
            if (event.target.checked) {
                num = 1;
                call = [clickToggle.id, num.toString()]; 
            } else{
                num = 0;
                call = [clickToggle.id, num.toString()]; 
            }
            webpage.emit('returningRPCName', call);
        })
    })

    setTimeout(() => {
        //Push Sensor information to display
        const deviceinfo = document.getElementById('sensorinfo');
        const display = document.createElement('info');
        display.type = "paragraph";
        display.innerText= serial[0];
        deviceinfo.appendChild(display);

        //write out rpc divs
        const rpcGroups = new Map();
        rpcs.forEach(rpc => {
            const prefix = rpc.split('.').slice(0, -1).join('.');
            const suffix = rpc.split('.').slice(-1).join('.');
            if (!rpcGroups.has(prefix)) {
                rpcGroups.set(prefix, []);
            }
            rpcGroups.get(prefix).push(suffix);
        })

        const rpcsContainer = document.getElementById('RPCCommands')
        rpcGroups.forEach((commands, prefix) => {
            const rpcDiv = document.createElement('div');
            rpcDiv.id = prefix;
            rpcDiv.className = 'controls'
            rpcDiv.style.display = 'none';

            const title = document.createElement('paragraph');
            title.innerText = prefix + ' ';
            rpcDiv.appendChild(title);

            commands.forEach(command => {
                let addElement;
                if (command === 'enable') {
                    addElement = document.createElement('input');
                    addElement.type = 'checkbox';
                    addElement.className = "checkCommands";
                } else if (command === 'reset') {
                    addElement = document.createElement('button');
                    addElement.innerText = 'Reset';
                    addElement.className = "buttonCommands";
                } else {
                    addElement = document.createElement('input');
                    addElement.type = 'number';
                    addElement.step = 0.1;
                    addElement.className = "InputCommands";
                }
                addElement.id = `${prefix}.${command}`;

                if (command != 'reset'){
                    const label = document.createElement('label');
                    label.htmlFor = addElement.id;
                    label.innerText = command + ' '
                    label.appendChild(addElement)
                    rpcDiv.appendChild(label);
                } else{rpcDiv.appendChild(addElement)}
                const lines = document.createElement('br');
                rpcDiv.appendChild(lines)
            })
            rpcsContainer.appendChild(rpcDiv)
        })

        const inputChange = document.querySelectorAll('.InputCommands');
        const toggleChange = document.querySelectorAll('.checkCommands') 

        //write out a chart for each column 
        let lastLabel = "none";
        const rpcType = document.querySelectorAll('.controls');
        for (let i = 0; i < columns.length; i++) {
            const checkboxesContainer = document.getElementById('dropdown');
            const canvasesContainer = document.getElementById('canvases');

            //create checkboxes
            const checkbox = document.createElement('input');
            checkbox.type = "checkbox";
            checkbox.id = `canvasCheckbox${i}`;
            checkbox.className = 'checkboxes'

            //create labels for checkboxes
            const label = document.createElement('label');
            label.htmlFor = checkbox.id;
            label.innerText = columns[i]
    
            const lineBreak = document.createElement('br');

            //create canvas
            const canvas = document.createElement('div');
            canvas.id = `canvas${i}`;
            canvas.classList = 'canvas-container';
            canvas.style.display = 'none';
            canvasesContainer.appendChild(canvas)

            //add objects to div
            checkboxesContainer.appendChild(checkbox);
            checkboxesContainer.appendChild(label);
            checkboxesContainer.appendChild(lineBreak);
            
            //event listener to display canvas/RPC div on click
            checkbox.addEventListener("change", (event) => {
                const canvas = document.getElementById(`canvas${i}`)
                canvas.style.display = event.target.checked ? 'block' : 'none';

                rpcType.forEach(rpcControl => {
                    const checkboxes = document.querySelectorAll('.checkboxes');
                    let stayDisplayed = false;
                    const rpcControlId = rpcControl.id.split('.').slice(0, 1);
                    checkboxes.forEach(checkbox => {
                        if (checkbox.labels[0].innerText.includes(rpcControlId) && checkbox.checked) {
                            stayDisplayed = true; 
                        }
                        rpcControl.style.display = stayDisplayed? 'inline-block' : 'none';
                    });
                });
            });
            rpcType.forEach(rpcDiv => {
                const rpcControlId = rpcDiv.id.split('.').slice(0, 1);
                if (label.innerText.includes(lastLabel)) {} //pass;
                else{
                    inputChange.forEach(rpccall => {
                        if (label.innerText.includes(rpcControlId) && rpccall.parentNode.parentNode == rpcDiv){
                            webpage.emit('onLoad', rpccall.id)
                            lastLabel = rpcDiv.id
                        } 
                    })
                    toggleChange.forEach(toggleChange => {
                        if (label.innerText.includes(rpcControlId) && toggleChange.parentNode.parentNode == rpcDiv){
                            webpage.emit('onLoad', toggleChange.id)
                        } 
                    })
                }
            })

            //uplot graph styling
            let options = {
                width: 800, 
                height: 300,
                series: [
                    {label: 'Time'},
                    { 
                        label: columns[i],
                        stroke: 'red',
                        points: { show: false },    
                        value: (u, v) => v  
    
                    },
                ],
                axes: [
                    {},
                    {
                        size: 80,
                        values: (u, v) => v
                    }
                ],
                scales: {
                    x: {
                    time: false,
                    distr: 2,
                    auto: true,
                    }
                }
            }

            const data = [[],[]]
            const uplot = new uPlot(options, data, document.getElementById(canvas.id))
            graphs.push(uplot)

            makeResizable(canvas.id, uplot)

            //display rpc values on load
            webpage.listen("returnOnLoad", (event) => {
                let [name, inputValue] = event.payload;
                inputChange.forEach(rpccall => {
                    if (rpccall.id == name) {
                        rpccall.value = inputValue;
                        rpccall.textContent = inputValue;
                        rpccall.innerHTML = inputValue;
                    }
                })
                toggleChange.forEach(toggle => {
                    if (toggle.id == name && inputValue == 1){
                        toggle.checked = true;
                    }
                })
            });

            
        }
        
    }, 2000);

    //returns all rpc values to corresponding input field
    webpage.listen("returnRPC", (event) => {
        let [name, inputValue] = event.payload;
        inputChange.forEach(rpccall => {
            if (rpccall.id == name) {
                rpccall.value = inputValue;
                rpccall.textContent = inputValue;
                rpccall.innerHTML = inputValue;
            }
        })
    }); 
    
    //SIDE BAR LOGIC
    const drop = document.getElementById('drop')
    drop.addEventListener("click", function() {
        const content = document.getElementById('dropdown')
        content.classList.toggle("show");
    })

    const pop = document.getElementById('showPlot')    
    pop.addEventListener("change", function() {
        const fftDiv = document.getElementById('FFT')
        if (pop.checked){
            fftDiv.style.display = 'inline-block'
        } else{ fftDiv.style.display = 'none'};
    })

    //FFT DIV
    webpage.once("fftgraphs", (event) => {
        const graphs = event.payload;
        graphs.forEach((graph, index) => {
            createFFTGraph(graph, `transform${index+1}`);
        })
    })
};

//test if input is within specified range
function in_range(fillValue) {
    const value = parseFloat(fillValue.value);
    const min = parseFloat(fillValue.min)
    const max = parseFloat(fillValue.max)

    if (isNaN(value) || value < min) {
        withinRange = min;
    } else if (value > max) {
        withinRange = max;
    } 
    else {withinRange = value;}
    return withinRange
}

// Create new FFT
function createFFTGraph(eventName, containerId) {
    const template = document.getElementById('fft-template');
    const clone = template.content.cloneNode(true);
    const container = clone.querySelector('.canvas-container');
    container.id = containerId;
    document.getElementById('FFT').appendChild(container); 

    const fftPlot = new uPlot({
            title: eventName,
            width: 800, 
            height: 300,
            series: [
                {   label: "Frequency (Hz)"},
                { 
                    label: "Power Spectrum (V/√Hz)",
                    stroke: 'blue',
                    points: { show: false },    
                },
            ],
            scales: {
                x: {
                time: false,
                auto: true,
                distr: 3
                },
                y: {distr: 3}
            },
            axes: [
                {},
                {   size: 100,
                    values: (u, v) => v
                }]
    }, [[],[]], container);

    makeResizable(containerId, fftPlot);

    // Listen for the event and update the graph
    webpage.listen(eventName, (event) => {
      const [frequencies, powerSpectrum] = event.payload;
      for (let i = 0; i< frequencies.length; i++) {
        fftPlot.data[0].push(frequencies[i]) 
        fftPlot.data[1].push(powerSpectrum[i])
      }
      while (fftPlot.data[0].length > 1000){
        fftPlot.data[0].shift();
        fftPlot.data[1].shift();
      }
      fftPlot.setData(fftPlot.data, true);
    });
}

function makeResizable(elementId, uplotInstance) {
    const element = document.getElementById(elementId);
    interact(element).resizable({
        edges: { left: false, right: false, bottom: true, top: true },
        inertia: true,
        listeners: {
        move(event) {
            const target = event.target;
            let width = event.rect.width;
            let height = event.rect.height;

            target.style.width = `${width}px`;
            target.style.height = `${height}px`;

            uplotInstance.setSize({ width, height });
        }
        },
        modifiers: [
        interact.modifiers.restrict({
            restriction: 'parent'
        }),
        interact.modifiers.restrictSize({
            min: { width: 400, height: 200 }
        })
        ]
    });
}

