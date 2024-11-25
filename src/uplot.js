const { invoke } = window.__TAURI__.tauri

invoke('graphs');

window.onload = () => {
    var graphs = []; //store graphs for display
    var columns = []; //store names for canvases
    var serial = [];

    let startTime = Date.now();
    //let seconds = startTime.getSeconds()
       
    gotNames = false;
    window.__TAURI__.event.listen('graphing', (event) => {
        const [values, name, info] = event.payload;
        const elapsed = (Date.now() - startTime) /1000;
        console.log(elapsed)
        
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

            maxPoints = 20;
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

    /*
    const toggleAll = document.getElementById('all')
    toggleAll.addEventListener("click", (source) => {
        canvas = document.getElementsByTagName('canvas')
        for ( let i = 0; i <canvas.length; i++)
            canvas[i].style.display = source.target.checked ? 'block': 'none';

    })*/


};


    