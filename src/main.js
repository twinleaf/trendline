const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWebviewWindow } = window.__TAURI__.webviewWindow;
const { once } = window.__TAURI__.event;

invoke('stream_data');
invoke('fft_data')
const webpage = getCurrentWebviewWindow();

const column_desc ={
    column: [],
    column_id: [],
    units: [],
    stream_num: []
}
const graphsByStream = {};
var rpcs = [];
var serial = []; 
let labelLoaded = false;
let rpcLoaded = false;

webpage.once("graph_labels", (event) => {
    const [header, label] = event.payload;
    serial.push(header.split('\n').slice(0)[0]);
    serial.push(header.split('\n').slice(1, 3).join('\n'));
    for (let name in label.col_name){
        column_desc.column.push(label.col_desc[name])
        column_desc.column_id.push(label.col_name[name])
        column_desc.units.push(label.col_unit[name])
        column_desc.stream_num.push(label.col_stream[name])
    }

    labelLoaded = true
})

webpage.once("rpcs", (event) => {
    const controls = event.payload;
    for (const group in controls) {
        rpcs.push(controls[group])
    }
    rpcLoaded = true
})

//Massive timeout to construct page elements, could benefit from some restructuring
new Promise((resolve) => {
    const checkLoad = setInterval(() => {
        if (labelLoaded && rpcLoaded) {
            clearInterval(checkLoad);
            resolve();
        }
    }, 100);
}).then(() => {
    setTimeout(() => {
        //Push Sensor information to display
        document.getElementById('deviceName').innerText= serial[0];
        document.getElementById('serialinfo').innerText = serial[1];

        //write out rpc divs
        const rpcGroups = new Map();
        rpcs.forEach(rpc => {
            const prefix = rpc[0].split('.').slice(0, -1).join('.');
            const suffix = rpc[0].split('.').slice(-1).join('.');
            if (!rpcGroups.has(prefix)) {
                rpcGroups.set(prefix, []);
            }
            rpcGroups.get(prefix).push([suffix, rpc[1]]);
        })

        const rpcsContainer = document.getElementById('RPCCommands')

        for (const prefix of rpcGroups.keys()){
            const rpcDiv = document.createElement('div');
            rpcDiv.id = prefix;
            rpcDiv.className = 'controls'

            const title = document.createElement('paragraph');
            title.innerText = prefix + ' ';
            rpcDiv.appendChild(title);
            rpcDiv.appendChild(document.createElement('br'))

            const suffixgroup = rpcGroups.get(prefix);
            suffixgroup.forEach(([suffix, writeable]) => {
                let addElement;
                if (suffix === 'enable') {
                    addElement = document.createElement('input');
                    addElement.type = 'checkbox';
                    addElement.className = "checkCommands";
                } else if (!writeable){
                    addElement = document.createElement('button');
                    addElement.innerText = suffix;
                    addElement.className = "buttonCommands";
                } else {
                    addElement = document.createElement('input');
                    addElement.type = 'number';
                    addElement.step = 0.1;
                    addElement.className = "InputCommands";
                }
                addElement.id = `${prefix}.${suffix}`;

                if (writeable){
                    const label = document.createElement('label');
                    label.htmlFor = addElement.id;
                    label.innerText = suffix + ' '
                    label.appendChild(addElement)
                    rpcDiv.appendChild(label);
                } else{rpcDiv.appendChild(addElement)}

                rpcDiv.appendChild(document.createElement('br'))
                
                rpcsContainer.appendChild(rpcDiv)
            })
        }

        attachInputListeners();
        attachToggleListeners();
        attachButtonListeners();
        streamsRPCSetup(); //setting up charts for RPC
        
        //Create stream graph listeners
        for (let i = Math.min(...column_desc.stream_num); i <= Math.max(...column_desc.stream_num); i++) {
            streamGraphs(i.toString());
        }

        //FFT Plot
        let opt = {
            title: `Power spectrum`,
            width: 800,
            height: 300,
            series: [{label: "Frequency (Hz)"}, 
                    {label: "Spectrum (1/√Hz)",
                    stroke: "blue",
                    points: {show: false}
                    }],
            scales: {
                x: {
                    time: false,
                    auto: true,
                    distr: 1
                },
                y: { distr: 3, 
                    auto: true
                 }
            },
            axes: [
                {},
                { size: 100, values: (u, v) => v }
            ]
        };
        let chart = new uPlot(opt, [[],[]], document.getElementById('FFT'));
        makeResizable('FFT', chart);

        window.addEventListener("resize", () =>{chart.setSize({ width: document.getElementById('FFT').clientWidth, height: 300});})

        let selection = document.getElementById('requestFFT')
        selection.addEventListener("change", ()=> {
            webpage.emit('fftName', selection.value)
            document.getElementById('FFT').style.display = 'block';
            chart.setSize({width: document.getElementById('FFT').clientWidth, height:300})
            let eventName = selection.value.split('.').join('').toString()
            webpage.listen(eventName, (event) => {
                const [freq, power] = event.payload;
                chart.data[0] = [];
                chart.data[1] = [];
                for (let i = 0; i< freq.length; i++){
                    chart.data[0].push(freq[i])
                    chart.data[1].push(power[i])
                }

                chart.setData(chart.data, true);
            })
        })
    }, 1000);
});

function streamsRPCSetup(){
    const inputChange = document.querySelectorAll('.InputCommands');
    const toggleChange = document.querySelectorAll('.checkCommands') 

    //write out a chart for each column 
    const rpcType = document.querySelectorAll('.controls');
    for (let i = 0; i < column_desc.column.length; i++) {
        const checkboxesContainer = document.getElementById('dropdown');
        const canvasesContainer = document.getElementById('canvases');

        const checkbox = document.createElement('input');
        checkbox.type = "checkbox";
        checkbox.id = column_desc.column_id[i]
        checkbox.className = 'checkboxes'

        const label = document.createElement('label');
        label.htmlFor = checkbox.id;
        label.innerHTML = column_desc.column[i]
        const lineBreak = document.createElement('br');

        const canvas = document.createElement('div');
        canvas.id = `canvas${i}`;
        canvas.classList = 'canvas-container';
        canvasesContainer.appendChild(canvas)

        checkboxesContainer.appendChild(checkbox);
        checkboxesContainer.appendChild(label);
        checkboxesContainer.appendChild(lineBreak);

        fftSelection = document.createElement('option')
        fftSelection.value = column_desc.column_id[i]
        fftSelection.innerHTML = column_desc.column_id[i]

        document.getElementById('requestFFT').appendChild(fftSelection)
        
        let options = {
            width: 800, 
            height: 300,
            series: [
                {label: 'Time'},
                { 
                    label: `${column_desc.column_id[i]}(${column_desc.units[i]})`,
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
        
        const streamNum = column_desc.stream_num[i];
        if (!graphsByStream[streamNum]) {
            graphsByStream[streamNum] = [];
        }
        graphsByStream[streamNum].push(uplot);
        makeResizable(canvas.id, uplot)

        //Setting canvas width to the size of the window, makeResizable element could just be removed if height specification is not needed
        window.addEventListener("resize", () =>{uplot.setSize({ width: canvas.clientWidth, height: 300});  })

        const checkboxes = document.querySelectorAll('.checkboxes');
        checkbox.addEventListener("change", (event) => {
            const canvas = document.getElementById(`canvas${i}`)
            canvas.style.display = event.target.checked ? 'block' : 'none';
            uplot.setSize({ width: canvas.clientWidth, height: 300});  
            
            rpcType.forEach(rpcControl => {
                let stayDisplayed = false;
                if (checkbox.id.split('.').slice(0,1).toString() == rpcControl.id.split('.').slice(0, 1).toString()) {
                    if (checkbox.checked){
                        stayDisplayed = true;
                        rpcControl.style.display = stayDisplayed? 'inline-block' : 'none';
                    }
                    else {
                        checkboxes.forEach(checkbox => { 
                            if (checkbox.id.split('.').slice(0,1).toString() == rpcControl.id.split('.').slice(0, 1).toString() && checkbox.checked){
                                stayDisplayed = true
                            }
                        })
                        rpcControl.style.display = stayDisplayed? 'inline-block' : 'none';
                    }
                }
            })
        });

        refreshRPC(checkbox)

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

    //Create stream graph listeners
    for (let i = Math.min(...column_desc.stream_num); i <= Math.max(...column_desc.stream_num); i++) {
        streamGraphs(i.toString());
    }

    //FFT Plot
    let opt = {
        title: `Power spectrum`,
        width: 800,
        height: 300,
        series: [{label: "Frequency (Hz)"}, 
                {label: "Spectrum (1/√Hz)",
                stroke: "blue",
                points: {show: false}
                }],
        scales: {
            x: {
                time: false,
                auto: true,
                distr: 3
            },
            y: { distr: 3 }
        },
        axes: [
            {},
            { size: 100, values: (u, v) => v }
        ]
    };
    let chart = new uPlot(opt, [[],[]], document.getElementById('FFT'));
    makeResizable('FFT', chart);

    let selection = document.getElementById('requestFFT')
    selection.addEventListener("change", ()=> {
        webpage.emit('fftName', selection.value)
        document.getElementById('FFT').style.display = 'block';
        let eventName = selection.value.split('.').join('').toString()
        webpage.listen(eventName, (event) => {
            const [freq, power] = event.payload;

            chart.data[0] = [];
            chart.data[1] = [];
            for (let i = 0; i< freq.length; i++){
                chart.data[0].push(freq[i])
                chart.data[1].push(power[i])
            }
            
            chart.setData(chart.data, true);
        })
    });
};

function streamGraphs(eventName){
    let timePoints = 100;
    let startTime = Date.now();

    webpage.listen(eventName, (event) => {
        const values = event.payload;
        const elapsed = (Date.now() - startTime) / 1000;
        const streamNum = parseInt(eventName);
        const startIndex = column_desc.stream_num.findIndex(num => num === streamNum);

        if (graphsByStream[streamNum]) {
            graphsByStream[streamNum].forEach((chart, index) => {
                const columnIndex = startIndex + index;
                for (let i = 0; i < values[index].length; i++) {
                    chart.data[0].push(elapsed);
                    chart.data[1].push(values[index][i]);

                    document.querySelectorAll('.checkboxes').forEach(checkbox => {
                        checkbox.labels.forEach(label => {
                            let nameParts = column_desc.column[columnIndex].split(' ');
                            const title = label.innerHTML.split(' ').slice(0, nameParts.length).join(' ');
                            if (column_desc.column[columnIndex] == title && column_desc.stream_num[columnIndex] == streamNum) {
                                let valueDisplay = values[index][i]
                                if (!isNaN(values[index][i]) && values[index][i] !== null && values[index][i] % 1 !== 0) {
                                    valueDisplay = values[index][i].toFixed(4);}

                                label.innerHTML = column_desc.column[columnIndex] + ' ' + valueDisplay
                            }
                        });
                    });

                    let firstLogTime = chart.data[0][0];
                    let recentLogTime = chart.data[0][chart.data[0].length - 1];

                    if ((recentLogTime - firstLogTime) > timePoints) {
                        for (let i = 0; i < chart.data.length; i++) {
                            chart.data[i].shift();
                        }
                    }
                }
                const timeSpan = document.getElementById('timeSpan');
                timeSpan.addEventListener('keypress', function(e) {
                    if (e.key == "Enter") {
                        timePoints = in_range(timeSpan);
                        let firstLogTime = chart.data[0][0];
                        let recentLogTime = chart.data[0][chart.data[0].length - 1];
                        while ((recentLogTime - firstLogTime) > timePoints) {
                            for (let i = 0; i < chart.data.length; i++) {
                                chart.data[i].shift();
                            }
                            firstLogTime = chart.data[0][0];
                            recentLogTime = chart.data[0][chart.data[0].length - 1];
                        }
                        timeSpan.innerHTML = timePoints;
                        timeSpan.value = timePoints;
                    }
                });
                chart.setData(chart.data, true);
            });
        }
    });
}

function makeResizable(elementId, uplotInstance) {
    const element = document.getElementById(elementId);
    interact(element).resizable({
        edges: { left: true, right: true, bottom: true, top: true },
        inertia: true,
        listeners: {
        move(event) {
            const target = event.target;
            let width = event.rect.width;
            let height = event.rect.height;

            target.style.width = `${width}px`;
            target.style.height = `${height+50}px`;

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

function refreshRPC(checkbox) {
    const rpcType = document.querySelectorAll('.controls');
    const inputChange = document.querySelectorAll('.InputCommands');
    const toggleChange = document.querySelectorAll('.checkCommands');
    rpcType.forEach(rpcDiv => {
        inputChange.forEach(rpccall => {
            if (checkbox.id.split('.').slice(0,1).toString() == rpcDiv.id.split('.').slice(0, 1) && rpccall.parentNode.parentNode == rpcDiv){
                webpage.emit('onLoad', rpccall.id)
            } 
        })
        toggleChange.forEach(toggleChange => {
            if (checkbox.id.split('.').slice(0,1).toString() == rpcDiv.id.split('.').slice(0, 1) && toggleChange.parentNode.parentNode == rpcDiv){
                webpage.emit('onLoad', toggleChange.id)
            } 
        })
    })

    webpage.listen("returnRPC", (event) => {
        let [name, inputValue] = event.payload;
        document.querySelectorAll('.InputCommands').forEach(rpccall => {
            if (rpccall.id == name) {
                rpccall.value = inputValue;
                rpccall.textContent = inputValue;
                rpccall.innerHTML = inputValue;
            }
        })
    }); 
}

function attachInputListeners() {
    const inputChange = document.querySelectorAll('.InputCommands');
    inputChange.forEach(rpccall => {
        rpccall.addEventListener('keypress', function(e) {
            value = in_range(rpccall).toString();
            if (e.key == "Enter") {
                call = [rpccall.id, value];
                webpage.emit('returningRPCName', call); 

                document.querySelectorAll('.checkboxes').forEach(checkbox => {
                    refreshRPC(checkbox)
                })
            }
        });
    });
}

function attachToggleListeners() {
    const toggleChange = document.querySelectorAll('.checkCommands');
    toggleChange.forEach(clickToggle => {
        clickToggle.addEventListener("change", (event) => {
            let num = 0;
            if (event.target.checked) {num = 1;}
            call = [clickToggle.id, num.toString()];
            webpage.emit('returningRPCName', call);

            document.querySelectorAll('.checkboxes').forEach(checkbox => {
                refreshRPC(checkbox)
            })
        });
    });
}

function attachButtonListeners() {
    const buttonChange = document.querySelectorAll('.buttonCommands');
    buttonChange.forEach(button => {
        button.addEventListener("click", () => {
            webpage.emit("onLoad", button.id);
        })
    })  
}

