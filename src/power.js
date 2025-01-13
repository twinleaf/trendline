const { listen } = window.__TAURI__.event;
const { getCurrentWebviewWindow } = window.__TAURI__.webviewWindow;
const { invoke } = window.__TAURI__.core;

webpage = getCurrentWebviewWindow();
window.onload = () => {
    var graphs = []; //store graphs for display
    var columns = []; //store names for canvases
    var serial = [];
    let timePoints = 10;

    let startTime = Date.now();
    gotNames = false;

    webpage.listen("stream-3", (event) => {
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

    setTimeout(() => {
        //Push Sensor information to display
        const deviceinfo = document.getElementById('sensorinfo');
        const display = document.createElement('info');
        display.type = "paragraph";
        display.innerText= serial[0];

        deviceinfo.appendChild(display);

        //Iterate values for all charts 
        for (let i = 0; i < columns.length; i++) {
            const canvasesContainer = document.getElementById('canvases');

            //create canvas
            const canvas = document.createElement('div');
            canvas.id = `canvas${i}`;
            canvas.classList = 'canvas-container';
            canvas.style.display = 'block';
            canvasesContainer.appendChild(canvas)

            //uplot graph styling
            let options = {
                width: 800, 
                height: 300,
                series: [
                    {
                        label: 'Time'
                    },
                    { 
                        label: columns[i],
                        stroke: 'red',
                        points: { show: false },
                        spanGaps: false,      
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
                    },
                },
            }

            const data = [[],[]]
            const uplot = new uPlot(options, data, document.getElementById(canvas.id))
            graphs.push(uplot)

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

    }, 1000);

    //Popout Window
    const pop = document.getElementById('pop')
    pop.addEventListener("click", function() {
        pop.disabled = true;
        invoke('create_window')
            .then(() => {console.log("window created")})
            .catch((error) => {console.log("error", error)})
    });

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


    