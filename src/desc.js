import FFT from "fft.js"

const f = new FFT(1024)
const input = new Array(1024)
for (let i = 0; i< input.length; i++ ) {
    input[i] = Math.sin(2*Math.PI * i /16)
}

const out = f.createComplexArray();
f.transform(out, input);
const magnitudes = new Array(out.length /2);
for (let i = 0; i< magnitudes.length; i++) {
    magnitudes[i] = Math.sqrt(out[i*2] * out[i*2] + out[i*2+ 2] * out[i * 2 + 1]);
}

const display = document.getElementById('display')
display.textContent = magnitudes;
