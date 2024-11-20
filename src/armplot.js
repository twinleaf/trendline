const { invoke } = window.__TAURI__.tauri

invoke('graphs');

window.onload = () => {
    var graphs = []; //store graphs for display

    let startTime = new Date();
    const start = document.getElementById('start')
    const stop = document.getElementById('pause')
    let isPaused = true;

    stop.addEventListener("click", function() {
        stop.disabled = true;
        isPaused = true;
        start.disabled = false; 
    })

    start.addEventListener("click", function() {
        start.disabled = true;
        isPaused = false;
        stop.disabeled = false;
    })
       
    gotNames = false;
    window.__TAURI__.event.listen('graphings', (event) => {
        const [values] = event.payload;
        const elapsed = (Date.now() - startTime) /1000;

        if (!isPaused) {
            addData(values, elapsed)
        }

        function addData(data, time) {
            graphs.forEach((chart) => { //iterate through each graph
    
                chart.data[0].push(time)
                chart.data[1].push(data)
    
                maxPoints = 10;
                if (chart.data[0].length > maxPoints){
                    chart.data[0].shift();
                    chart.data[1].shift();
                }
    
                chart.setData(chart.data);
                chart.redraw(true, true);
            })
            
        }
    });

    setTimeout(() => {
        //Iterate values for all charts (currently hard code to # of charts)
        for (let i = 0; i < 1; i++) {

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
            label.innerText = "Temp";

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
                height: 600,
                series: [
                    {   
                        value: (u, v) => v,
                    },
                    { 
                        label: "Temp",
                        stroke: 'red',
                        points: { show: false },         
    
                    },
                ],
                axes: [
                    {},
                    {
                        tick: {show: true,},
                        grid: {show: true}
                    }
                ],
                scales: {
                    x: {
                    distr: 2,
                    }
                }
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
                        min: {width: 300, height: 200}
                    })
                ]
            })
        }

    }, 100);
    
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


    