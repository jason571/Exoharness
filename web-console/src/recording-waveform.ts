// Local microphone metering only. No audio output, extra capture, or upload.
export function mountRecordingWaveform(panel: HTMLElement | null) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.classList.add("speech-waveform");
  svg.setAttribute("viewBox", "0 0 308 40");
  svg.setAttribute("preserveAspectRatio", "none");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("hidden", "");
  const bars = Array.from({ length: 44 }, (_, index) => {
    const bar = document.createElementNS(svg.namespaceURI, "rect");
    for (const [key, value] of Object.entries({ x: String(index * 7 + 1), y: "18.5", width: "4", height: "3", rx: "2" })) bar.setAttribute(key, value);
    svg.append(bar);
    return bar;
  });
  panel?.insertBefore(svg, panel.querySelector("button"));
  let release: (() => void) | null = null;
  function stop() { release?.(); release = null; svg.setAttribute("hidden", ""); }
  function start(stream: MediaStream) {
    stop();
    const Constructor = window.AudioContext || (window as Window & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!panel || !Constructor) return; // Meter failure must not disable dictation.
    let context: AudioContext | undefined;
    let source: MediaStreamAudioSourceNode | undefined;
    let analyser: AnalyserNode | undefined;
    let frame = 0;
    let disposed = false;
    const cleanup = () => {
      disposed = true;
      cancelAnimationFrame(frame);
      source?.disconnect(); analyser?.disconnect();
      if (context && context.state !== "closed") void context.close().catch(() => {});
      svg.setAttribute("hidden", "");
      // Stream tracks belong to MediaRecorder; stopping the meter cannot stop capture.
    };
    release = cleanup;
    try {
      context = new Constructor();
      source = context.createMediaStreamSource(stream);
      analyser = context.createAnalyser();
      analyser.fftSize = 1024;
      source.connect(analyser); // Never connect to the speakers (no feedback).
      const samples = new Float32Array(analyser.fftSize);
      const levels = new Array<number>(bars.length).fill(0);
      let last = -Infinity;
      const interval = matchMedia("(prefers-reduced-motion: reduce)").matches ? 120 : 50;
      const draw = (now: number) => {
        if (disposed || !analyser) return;
        if (now - last >= interval) {
          last = now;
          analyser.getFloatTimeDomainData(samples);
          const rms = Math.sqrt(samples.reduce((sum, value) => sum + value * value, 0) / samples.length);
          levels.shift(); levels.push(rms < 0.003 ? 0 : Math.min(1, Math.sqrt(rms) * 2.5));
          bars.forEach((bar, index) => {
            const height = 3 + levels[index] * 33;
            bar.setAttribute("y", String((40 - height) / 2));
            bar.setAttribute("height", String(height));
          });
        }
        frame = requestAnimationFrame(draw);
      };
      void context.resume().then(() => {
        if (disposed) return;
        svg.removeAttribute("hidden");
        frame = requestAnimationFrame(draw);
      }).catch(cleanup);
    } catch { cleanup(); }
  }
  return { start, stop };
}
