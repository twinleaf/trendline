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
const complexPlots = []
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

webpage.once("fftgraphs", (event) => {
    const [real, complex] = event.payload;
    //TODO: reduce verbosity
    for (let keys in real){    
        let labels = [];        
        for (let value in real[keys] ){
            labels.push(real[keys][value])
        }
        createFFT(keys, `${keys}`, labels)   
    }
    for (let keys in complex){
        let labels = [];
        for (let value in complex[keys]) {
            labels.push(complex[keys][value])
            complexPlots.push(complex[keys][value])
        }
        createFFT(`${complex[keys][0].split('.').slice(0,1)}${keys}`, labels)   
    }
})

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
            rpcDiv.style.display = 'none';

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
            canvas.style.display = 'none';
            canvasesContainer.appendChild(canvas)

            checkboxesContainer.appendChild(checkbox);
            checkboxesContainer.appendChild(label);
            checkboxesContainer.appendChild(lineBreak);
            
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

            const checkboxes = document.querySelectorAll('.checkboxes');

            checkbox.addEventListener("change", (event) => {
                const canvas = document.getElementById(`canvas${i}`)
                canvas.style.display = event.target.checked ? 'block' : 'none';
                
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

        for (let i = Math.min(...column_desc.stream_num); i <= Math.max(...column_desc.stream_num); i++) {
            streamGraphs(i.toString());
        }

        const ffts = document.getElementById('showPlot')
        ffts.addEventListener("change", () => {
            document.getElementById('FFT').childNodes.forEach(node => {
                if (ffts.checked){
                    let stayDisplayed = false;
                    document.querySelectorAll('.checkboxes').forEach(checkbox => {
                        let idParts = checkbox.id.split('.');
                        //again need better way to identify complex plot
                        if (complexPlots.includes(checkbox.id) && `${idParts.slice(0,1).toString()}${idParts.slice(-1).join().charAt(0)}` == node.id.toString() && checkbox.checked) {
                            stayDisplayed = true;
                            return;
                        } else if (idParts.slice(0,1).toString() == node.id.toString() && checkbox.checked) {
                            stayDisplayed = true;
                            return;
                        }
                    })
                    document.getElementById('FFT').style.display = 'block';
                    node.style.display = stayDisplayed? 'block' : 'none';
                } else {document.getElementById('FFT').style.display = 'none'}
            })
        })

        /*document.getElementById('requestFFT').addEventListener("keypress", function(e) {
            if (e.key == "Enter") {webpage.emit('FFT request', document.getElementById('requestFFT').value)}
        })*/
    }, 1000);
});

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

function createFFT(eventName, labels) {
    const template = document.getElementById('fft-template');
    const clone = template.content.cloneNode(true);
    const container = clone.querySelector('.canvas-container');
    container.id = eventName;
    let fftPlot;

    let seriesConfig  = [{label: "Frequency (Hz)"}];
    let gotSeries = false;

    webpage.listen(eventName, (event) => {
        const spectrum = event.payload;
        if (!gotSeries) {
            for (let i = 1; i< spectrum.length; i++) {
                seriesConfig.push({
                    label: `${labels[i-1]} (${column_desc.units[i-1]}/√Hz)`, 
                    stroke: `hsl(${i*130}, 30%, 35%)`,
                    points: {show: false}
                })
            } 
            gotSeries = true;
        }
        //TODO Freeze resolve
        new Promise((resolve) => {
            const plotCreated = setInterval(() => {
                if (fftPlot !== undefined) {
                    clearInterval(plotCreated);
                    resolve();
                }
            }, 100);
        }).then(() => {
            for (let i = 0; i< spectrum[0].length; i++){
                for (let j = 0; j< spectrum.length; j++){
                    if (spectrum[j][i] !== undefined) {
                        fftPlot.data[j].push(spectrum[j][i])
                    }
                }
            }

            while (fftPlot.data[0].length > 500){
                for (let i = 0; i < fftPlot.data.length; i++){
                    fftPlot.data[i].shift();
                }
            }
            fftPlot.setData(fftPlot.data, true);
        })
        
    });
    document.getElementById('FFT').appendChild(container);
    new Promise((resolve) => {
        const checkSeriesConfig = setInterval(() => {
            if (gotSeries) {
                clearInterval(checkSeriesConfig);
                resolve();
            }
        }, 100);
    }).then(() => {
        setTimeout(() => {
            let data = Array.from({ length: seriesConfig.length }, () => []);
            let opt = {
                title: `${eventName} power spectrum`,
                width: 800,
                height: 300,
                series: seriesConfig,
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
            let chart = new uPlot(opt, data, container);
            fftPlot = chart;
            makeResizable(container.id, chart);
        }, 500);
    })
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

