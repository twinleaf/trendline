const { invoke } = window.__TAURI__.core
const { listen } = window.__TAURI__.event;
const { getCurrentWebviewWindow } = window.__TAURI__.webviewWindow;

invoke('graphs');

webpage = getCurrentWebviewWindow();

window.onload = () => {
    var graphs = []; //store graphs for display
    var columns = []; //store names for canvases
    var serial = []; //store device serial information
    let timePoints = 50;
    const inputChange = document.querySelectorAll('.InputCommands');
    const rpcType = document.querySelectorAll('.controls');
    
    let startTime = Date.now();
    gotNames = false; //bool determines if graph information is set up

    //emit rust data and push to uplot graphs 
    webpage.listen("graphing", (event) => {
        const elapsed = (Date.now() - startTime) /1000;
        const [values, name, header] = event.payload;
        
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

            const timeSpan = document.getElementById('timeSpan');
            timeSpan.addEventListener('keypress', function(e) { //adjust graph time span
                if (e.key == "Enter") {
                    timePoints = in_range(timeSpan);
                    chart.setScale('x',{min: 1, max: timePoints});
                    if (chart.data[1].length> timePoints) {
                        while (chart.data[0].length > timePoints) {
                            chart.data[0].shift();
                            chart.data[1].shift();
                            chart.setData(chart.data);
                            chart.redraw();
                        }
                    } else{chart.redraw()
                        console.log(chart.data[1].length)
                    }; 
                    timeSpan.innerHTML = timePoints;
                    timeSpan.value = timePoints;
                }
            }) 

            let maxPoints = timePoints;
            if (chart.data[1].length > maxPoints){
                chart.data[0].shift();
                chart.data[1].shift();
            }
            console.log("moving", maxPoints, chart.data[0].length, chart.data[1].length)
            chart.setData(chart.data);
            chart.redraw();
        }) 
    });

    setTimeout(() => {
        //display rpc values on load
        inputChange.forEach(rpccall => {
            webpage.emit('onLoad', rpccall.id);    
        })

        webpage.listen("returnOnLoad", (event) => {
            let [name, inputValue] = event.payload;
            inputChange.forEach(rpccall => {
                if (rpccall.id == name) {
                    rpccall.value = inputValue;
                    rpccall.textContent = inputValue;
                    rpccall.innerHTML = inputValue;
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
            label.innerText = columns[i];

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
            
            //event listener to display canvas on click
            checkbox.addEventListener("change", (event) => {
                const canvas = document.getElementById(`canvas${i}`)
                canvas.style.display = event.target.checked ? 'block' : 'none';

                //TODO: fix those logic where on a change it somehow has to check all the instances
                rpcType.forEach(rpcControl => {
                    if (label.innerText.includes(rpcControl.id)) {
                        rpcControl.style.display = event.target.checked? 'inline-block' : 'none';
                    }
                });
            });

            //uplot graph styling
            let options = {
                width: 800, 
                height: 300,
                series: [
                    {   
                        value: (u, v) => v,
                    },
                    { 
                        label: columns[i],
                        stroke: 'red',
                        points: { show: false },
                        spanGaps: true,         
    
                    },
                ],
                scales: {
                    x: {
                    time: false,
                    distr: 2,
                    range: (min, max) => [min, max]
                    }
                }
            }

            const data = [[],[]]
            const uplot = new uPlot(options, data, document.getElementById(canvas.id))
            uplot.setScale('x',{min: 1, max: timePoints});
            graphs.push(uplot)

            //interact js to resize chart height
            const targetElement = document.getElementById(canvas.id)
            const targetResize = interact(targetElement);

            targetResize.resizable({
                edges: {left: false, right: false, bottom: true, top:true},
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
                        min: {height: 200}
                    })
                ]
            })

        }

    }, 1000);
    
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
                //TODO: Determine whether to simply make the id the rpc name 
                //or it needs to be something more private
                call= [rpccall.id, value];
                webpage.emit('returningRPCName', call);    
            }
        })
    })

    //on checkbox change call rpc command
    const toggleChange = document.querySelectorAll('.checkCommands') 
    toggleChange.forEach(clickToggle => {
        clickToggle.addEventListener("click", function(e) {
            if (e.checked) {
                call = [clickToggle.id, clickToggle.value]; 
                webpage.emit('returningRPC', call);
            }
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
    } else {
        withinRange = value;
    }
    return withinRange
}

    