const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { Window } = window.__TAURI__.window;
const { Webview } = window.__TAURI__.webview;
const { getCurrentWebviewWindow } = window.__TAURI__.webviewWindow;

invoke('graph_data', {data: 'data'}).then((data) => {});

webpage = getCurrentWebviewWindow();
window.onload = () => {
    var graphs = []; //store graphs for display
    var columns = []; //store names for canvases
    var serial = []; 
    let timePoints = 10;
    const inputChange = document.querySelectorAll('.InputCommands');
    const toggleChange = document.querySelectorAll('.checkCommands') 
    const rpcType = document.querySelectorAll('.controls');
    
    let startTime = Date.now();
    gotNames = false;

    //emit rust data and push to uplot graphs 
    webpage.listen("aux", (event) => {
        const [values, name, header] = event.payload;
        const elapsed = (Date.now() - startTime) /1000;
        
        //push names to canvas
        if (!gotNames) {
            for (let i = 0; i< name.length; i++) {
                columns.push(name[i])
            }
            serial.push(header)
            gotNames = true;
        } 

        //push data to each graph
        graphs.forEach((chart, index) => {
            chart.data[0].push(elapsed)
            chart.data[1].push(values[index])

            let firstLogTime = chart.data[0][0]
            let recentLogTime = chart.data[0][chart.data[0].length -1] //last timestamp

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
    
            if ((recentLogTime - firstLogTime) > timePoints){
                chart.data[0].shift();
                chart.data[1].shift();
            }
            
            chart.setData(chart.data, true);
        }) 
    });

    //on button click return rpc
    const buttonChange = document.querySelectorAll('.buttonCommands');
    buttonChange.forEach(clickButton => {
        clickButton.addEventListener("click", function() {   
            call = [clickButton.id, clickButton.value];     
            webpage.emit('returningRPCName', call);      
        })
    })

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

    setTimeout(() => {
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
        
        //Push Sensor information to display
        const deviceinfo = document.getElementById('sensorinfo');
        const display = document.createElement('info');
        display.type = "paragraph";
        display.innerText= serial[0];
        deviceinfo.appendChild(display);

        //write out a chart for each column 
        let lastLabel = "none";
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
                    checkboxes.forEach(checkbox => {
                        if (checkbox.labels[0].innerText.includes(rpcControl.id) && checkbox.checked) {
                            stayDisplayed = true; 
                        }
                        rpcControl.style.display = stayDisplayed? 'inline-block' : 'none';
                    });
                });
            });
            rpcType.forEach(rpcDiv => {
                if (label.innerText.includes(lastLabel)) {} //pass;
                else{
                    inputChange.forEach(rpccall => {
                        if (label.innerText.includes(rpcDiv.id) && rpccall.parentNode.parentNode == rpcDiv){
                            webpage.emit('onLoad', rpccall.id)
                            lastLabel = rpcDiv.id
                        } 
                    })
                    toggleChange.forEach(toggleChange => {
                        if (label.innerText.includes(rpcDiv.id) && toggleChange.parentNode.parentNode == rpcDiv){
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

            //interact js to resize chart height
            const targetElement = document.getElementById(canvas.id)
            const targetResize = interact(targetElement);

            targetResize.resizable({
                edges: {left: true, right: true, bottom: true, top:true},
                inertia: true,
                listeners: {
                    move(event) {
                        const target = event.target;
                        let width = event.rect.width;
                        let height = event.rect.height;

                        target.style.width = `${width}px`
                        target.style.height = `${height}px`

                        uplot.setSize({width, height});
                    }
                },
                modifiers: [
                    interact.modifiers.restrict({
                        restriction: 'parent'
                    }),
                    
                    interact.modifiers.restrictSize({
                        min: {width: 400, height: 200}
                    })
                ]
            })
        }
        
    }, 2000);
    
    //SIDE BAR LOGIC
    const drop = document.getElementById('drop')
    drop.addEventListener("click", function() {
        const content = document.getElementById('dropdown')
        content.classList.toggle("show");
    })

    //page tabbing
    document.querySelectorAll('.tabs div').forEach(tab => {
        tab.addEventListener('click', function() {
            //deactivate all visibility
            document.querySelectorAll('.tabs div').forEach(tab => tab.classList.remove('active'));
            //activate specified tab
            tab.classList.add('active');
            
            const webviewShow = tab.id;
            webpage.emit('toggle', webviewShow)
        })
    })    
};

//function tests if input is within specified range
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

    