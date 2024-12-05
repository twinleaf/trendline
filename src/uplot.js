const { invoke } = window.__TAURI__.core
const { listen } = window.__TAURI__.event;
const { getCurrentWebviewWindow } = window.__TAURI__.webviewWindow;

invoke('graphs');

webpage = getCurrentWebviewWindow();

window.onload = () => {

    var graphs = []; //store graphs for display
    var columns = []; //store names for canvases
    var serial = [];
    let points = 20;

    let startTime = Date.now();
       
    gotNames = false;

    webpage.listen("graphing", (event) => {
        const [values, name, info] = event.payload;
        const elapsed = (Date.now() - startTime) /1000;
        
        //push names to canvas
        if (!gotNames) {
            for (let i = 0; i< name.length; i++) {
                columns.push(name[i])
            }
            serial.push(info)
            gotNames = true;
        } 

        graphs.forEach((chart, index) => { //iterate through each graph
            chart.data[0].push(elapsed)
            chart.data[1].push(values[index])

            //TODO: Graph shifting as points filter in on change
            let maxPoints = points;
            if (chart.data[0].length > maxPoints){
                chart.data[0].shift();
                chart.data[1].shift();
            }
            chart.setData(chart.data);
            chart.redraw(true, true);
        }) 
    });

    setTimeout(() => {
        //Push Sensor information to display
        const deviceinfo = document.getElementById('sensorinfo');
        const display = document.createElement('info');
        display.type = "paragraph";
        display.innerText= serial[0];

        deviceinfo.appendChild(display);

        //Iterate values for all charts 
        for (let i = 0; i < columns.length; i++) {
            const checkboxesContainer = document.getElementById('please');
            const canvasesContainer = document.getElementById('canvases');

            //create check boxes general format 
            const checkbox = document.createElement('input');
            checkbox.type = "checkbox";
            checkbox.id = `canvasCheckbox${i}`;
            checkbox.className = 'checkboxes'
            checkbox.value = i;

            //create label for checkboxes
            const label = document.createElement('label');
            label.htmlFor = checkbox.id;
            label.innerText = columns[i];

            //create canvas
            const canvas = document.createElement('div');
            canvas.id = `canvas${i}`;
            canvas.classList = 'canvas-container';
            canvas.style.display = 'none';
            canvasesContainer.appendChild(canvas)

            checkboxesContainer.appendChild(checkbox);
            checkboxesContainer.appendChild(label);

            
            //event listener to display canvas
            checkbox.addEventListener("change", (event) => {
                const canvas = document.getElementById(`canvas${i}`)
                canvas.style.display = event.target.checked ? 'block' : 'none';
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
                        spanGaps: false,         
    
                    },
                ],
                axes: [
                    { },
                    {
                        tick: {show: true,},
                        grid: {show: true}
                    }
                ],
                scales: {
                    x: {
                    time: true,
                    min:0,
                    max:1000,
                    distr: 2,
                    auto: true,
                    },
                }
            }

            const data = [[],[]]

            const uplot = new uPlot(options, data, document.getElementById(canvas.id))
            graphs.push(uplot)

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
        const content = document.getElementById('please')
        content.classList.toggle("show");
    })

    //page tabbing logic
    document.querySelectorAll('.tabs div').forEach(tab => {
        tab.addEventListener('click', function() {
            //deactivate visibility
            document.querySelectorAll('.tabs div').forEach(tab => tab.classList.remove('active'));
            
            //activate tab visibility
            tab.classList.add('active');
            
            const webviewShow = tab.id;
            webpage.emit('toggle', webviewShow)
             
        })
    })

    const timeSpan = document.getElementById('timeSpan');
    timeSpan.addEventListener('input', () => {
        const value = parseFloat(timeSpan.value);
        const min = parseFloat(timeSpan.min)
        const max = parseFloat(timeSpan.max)

        if (isNaN(value) || value < min) {
            points = min;
        } else if (value > max) {
            points = max;
        } else {
            points = value;
        }
    })

};


    