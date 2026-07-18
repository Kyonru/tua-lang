"use strict";

function spritePreviewRuntime(sprites) {
  const serializedSprites = JSON.stringify(sprites).replace(/</g, "\\u003c");

  return `    const sprites = ${serializedSprites};
    const root = document.getElementById("sprites");
    const summary = document.getElementById("summary");
    const reloadButton = document.getElementById("reload-preview");
    const vscode = typeof acquireVsCodeApi === "function" ? acquireVsCodeApi() : undefined;
    const animationCount = sprites.reduce((total, sprite) => total + sprite.animations.length, 0);

    summary.appendChild(pill(sprites.length + " sprite" + (sprites.length === 1 ? "" : "s")));
    summary.appendChild(pill(animationCount + " animation" + (animationCount === 1 ? "" : "s")));
    reloadButton.addEventListener("click", () => {
      reloadButton.disabled = true;
      reloadButton.textContent = "Reloading";

      if (vscode) {
        vscode.postMessage({ command: "reload" });
      } else {
        window.location.reload();
      }
    });

    for (const sprite of sprites) {
      root.appendChild(createSpriteView(sprite));
    }

    function pill(text) {
      const element = document.createElement("span");
      element.className = "pill";
      element.textContent = text;
      return element;
    }

    function createSpriteView(sprite) {
      const section = document.createElement("section");
      section.className = "sprite";
      section.innerHTML = \`
        <div class="sprite-head">
          <div>
            <div class="name"></div>
            <div class="path"></div>
          </div>
          <div class="pill"></div>
        </div>
      \`;

      section.querySelector(".name").textContent = sprite.name;
      section.querySelector(".path").textContent = sprite.imagePath;
      section.querySelector(".pill").textContent = sprite.frame
        ? sprite.frame.width + "x" + sprite.frame.height
        : "Invalid frame";

      if (sprite.diagnostics.length > 0) {
        const problems = document.createElement("div");
        problems.className = "problems";

        for (const diagnostic of sprite.diagnostics) {
          const problem = document.createElement("div");
          problem.className = "problem error";
          problem.textContent = (diagnostic.code ? diagnostic.code + " " : "") + diagnostic.message;
          problems.appendChild(problem);
        }

        section.appendChild(problems);
      }

      if (!sprite.frame) {
        appendUnavailableMessage(section, "Sprite frame metadata is incomplete.");
        return section;
      }

      if (sprite.animations.length === 0) {
        appendUnavailableMessage(section, "No valid sprite animations were found.");
        return section;
      }

      if (!sprite.imageUri) {
        appendUnavailableMessage(section, "Could not find " + sprite.imagePath);
        return section;
      }

      const workbench = document.createElement("div");
      workbench.className = "workbench";
      workbench.innerHTML = \`
        <div class="stage">
          <div class="stage-frame">
            <canvas class="sprite-canvas"></canvas>
          </div>
          <div class="stage-readout">
            <div class="readout"><span>Frame</span><span class="frame-value">-</span></div>
            <div class="readout"><span>Source rect</span><span class="rect-value">-</span></div>
          </div>
        </div>
        <div class="inspector">
          <div class="animation-tabs"></div>
          <div class="toolbar">
            <div class="transport">
              <button type="button" class="previous">Prev</button>
              <button type="button" class="primary play">Pause</button>
              <button type="button" class="next">Next</button>
            </div>
            <div class="adjustments">
              <label class="slider">Speed <input class="speed" type="range" min="0.25" max="2" step="0.25" value="1"><span class="speed-value">1x</span></label>
              <label class="slider">Zoom <input class="zoom" type="range" min="1" max="4" step="0.25" value="1"><span class="zoom-value">Fit</span></label>
            </div>
          </div>
          <div class="metrics">
            <div class="metric"><span>Range</span><span class="range-value">-</span></div>
            <div class="metric"><span>FPS</span><span class="fps-value">-</span></div>
            <div class="metric"><span>Mode</span><span class="mode-value">-</span></div>
            <div class="metric"><span>Sheet</span><span class="sheet-value">-</span></div>
          </div>
          <div class="strip">
            <div class="strip-head">
              <span>Frames</span>
              <span class="frame-count">-</span>
            </div>
            <div class="timeline"></div>
          </div>
          <div class="sheet">
            <div class="sheet-head">
              <span>Sprite sheet</span>
              <span class="sheet-meta">-</span>
            </div>
            <div class="sheet-viewport">
              <canvas class="sheet-canvas"></canvas>
            </div>
          </div>
        </div>
      \`;
      section.appendChild(workbench);

      const stageFrame = workbench.querySelector(".stage-frame");
      const canvas = workbench.querySelector(".sprite-canvas");
      const sheetCanvas = workbench.querySelector(".sheet-canvas");
      const tabs = workbench.querySelector(".animation-tabs");
      const previousButton = workbench.querySelector(".previous");
      const playButton = workbench.querySelector(".play");
      const nextButton = workbench.querySelector(".next");
      const speedInput = workbench.querySelector(".speed");
      const speedValue = workbench.querySelector(".speed-value");
      const zoomInput = workbench.querySelector(".zoom");
      const zoomValue = workbench.querySelector(".zoom-value");
      const frameValue = workbench.querySelector(".frame-value");
      const rectValue = workbench.querySelector(".rect-value");
      const rangeValue = workbench.querySelector(".range-value");
      const fpsValue = workbench.querySelector(".fps-value");
      const modeValue = workbench.querySelector(".mode-value");
      const sheetValue = workbench.querySelector(".sheet-value");
      const sheetMeta = workbench.querySelector(".sheet-meta");
      const frameCount = workbench.querySelector(".frame-count");
      const timeline = workbench.querySelector(".timeline");
      const context = canvas.getContext("2d");
      const sheetContext = sheetCanvas.getContext("2d");
      const image = new Image();
      const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      let animation = sprite.animations[0];
      let frameOffset = 0;
      let lastTime = 0;
      let playing = !reducedMotion;
      let speedScale = 1;
      let zoomScale = 1;

      for (const candidate of sprite.animations) {
        const tab = document.createElement("button");
        tab.type = "button";
        tab.textContent = candidate.name;
        tab.addEventListener("click", () => {
          animation = candidate;
          frameOffset = 0;
          playing = !reducedMotion;
          lastTime = 0;
          updateAll();
        });
        tabs.appendChild(tab);
      }

      previousButton.addEventListener("click", () => {
        stepFrame(-1);
      });

      nextButton.addEventListener("click", () => {
        stepFrame(1);
      });

      playButton.addEventListener("click", () => {
        playing = !playing;
        updatePlayButton();
      });

      speedInput.addEventListener("input", () => {
        speedScale = Number(speedInput.value) || 1;
        speedValue.textContent = speedScale + "x";
      });

      zoomInput.addEventListener("input", () => {
        zoomScale = Number(zoomInput.value) || 1;
        updateZoom();
      });

      window.addEventListener("resize", updateZoom);

      image.addEventListener("load", () => {
        updateCanvasDimensions();
        context.imageSmoothingEnabled = false;
        sheetContext.imageSmoothingEnabled = false;
        sheetValue.textContent = image.naturalWidth + "x" + image.naturalHeight;
        sheetMeta.textContent = image.naturalWidth + "x" + image.naturalHeight + " px";
        updateAll();
        requestAnimationFrame(tick);
      });

      image.addEventListener("error", () => {
        sheetMeta.className = "error";
        sheetMeta.textContent = "Could not load " + sprite.imagePath;
      });

      image.src = sprite.imageUri;

      function tick(time) {
        const fps = Math.max(0.001, animation.fps || 1);
        if (playing && time - lastTime >= 1000 / (fps * speedScale)) {
          lastTime = time;
          const frames = animationFrames(animation);
          if (animation.loop || frameOffset < frames.length - 1) {
            frameOffset = (frameOffset + 1) % frames.length;
          } else {
            playing = false;
          }
          updateAll();
        }

        requestAnimationFrame(tick);
      }

      function updateAll() {
        updateCanvasDimensions();
        updateTabs();
        updatePlayButton();
        updateZoom();
        updateMetrics();
        updateTimeline();
        drawFrame();
        drawSheet();
      }

      function updateTabs() {
        for (const tab of tabs.querySelectorAll("button")) {
          tab.classList.toggle("active", tab.textContent === animation.name);
        }
      }

      function updatePlayButton() {
        playButton.textContent = playing ? "Pause" : "Play";
      }

      function updateZoom() {
        zoomScale = Number(zoomInput.value) || zoomScale;

        const frame = animationFrame();
        const fitScale = containedFrameScale();
        const displayScale = fitScale * zoomScale;

        zoomValue.textContent = zoomScale === 1 ? "Fit" : Math.round(zoomScale * 100) + "%";
        canvas.style.width = Math.max(1, Math.floor(frame.width * displayScale)) + "px";
        canvas.style.height = Math.max(1, Math.floor(frame.height * displayScale)) + "px";
      }

      function containedFrameScale() {
        const frame = animationFrame();
        const availableWidth = Math.max(1, stageFrame.clientWidth - 24);
        const availableHeight = Math.max(1, stageFrame.clientHeight - 24);

        return Math.max(
          1,
          Math.min(availableWidth / frame.width, availableHeight / frame.height),
        );
      }

      function updateMetrics() {
        const frames = animationFrames(animation);
        const frame = currentFrame();
        const rect = sourceRect(frame);
        const geometry = animationFrame();

        frameValue.textContent = frame + " (" + (frameOffset + 1) + "/" + frames.length + ")";
        rectValue.textContent = rect.x + "," + rect.y + " " + geometry.width + "x" + geometry.height;
        rangeValue.textContent = animation.from + ".." + animation.to + ", row " + animation.row;
        fpsValue.textContent = animation.durations || (animation.fps + " fps");
        modeValue.textContent = animation.loop ? "Loop" : "Pause at end";
        frameCount.textContent = frames.length + " frame" + (frames.length === 1 ? "" : "s");
      }

      function stepFrame(direction) {
        const frames = animationFrames(animation);
        frameOffset = (frameOffset + direction + frames.length) % frames.length;
        playing = false;
        updateAll();
      }

      function drawFrame() {
        const rect = sourceRect(currentFrame());
        const frame = animationFrame();

        context.clearRect(0, 0, canvas.width, canvas.height);
        context.drawImage(
          image,
          rect.x,
          rect.y,
          frame.width,
          frame.height,
          0,
          0,
          canvas.width,
          canvas.height,
        );
      }

      function drawSheet() {
        const scale = Math.max(1, Math.min(8, Math.floor(320 / Math.max(image.naturalWidth, image.naturalHeight))));
        const rect = sourceRect(currentFrame());
        const frame = animationFrame();

        sheetCanvas.width = image.naturalWidth * scale;
        sheetCanvas.height = image.naturalHeight * scale;
        sheetCanvas.style.width = sheetCanvas.width + "px";
        sheetCanvas.style.height = sheetCanvas.height + "px";
        sheetContext.clearRect(0, 0, sheetCanvas.width, sheetCanvas.height);
        sheetContext.drawImage(image, 0, 0, sheetCanvas.width, sheetCanvas.height);
        sheetContext.strokeStyle = getComputedStyle(document.documentElement).getPropertyValue("--accent").trim() || "#ffffff";
        sheetContext.lineWidth = Math.max(2, scale);
        sheetContext.strokeRect(
          rect.x * scale + sheetContext.lineWidth / 2,
          rect.y * scale + sheetContext.lineWidth / 2,
          frame.width * scale - sheetContext.lineWidth,
          frame.height * scale - sheetContext.lineWidth,
        );
      }

      function updateTimeline() {
        const frames = animationFrames(animation);
        timeline.replaceChildren(...frames.map((frame, index) => {
          const entry = document.createElement("button");
          entry.type = "button";
          entry.className = index === frameOffset ? "frame-chip active" : "frame-chip";
          entry.textContent = String(frame);
          entry.addEventListener("click", () => {
            frameOffset = index;
            playing = false;
            updateAll();
          });
          return entry;
        }));
      }

      function currentFrame() {
        const frames = animationFrames(animation);
        return frames[Math.min(frameOffset, frames.length - 1)] || animation.from;
      }

      function sourceRect(frame) {
        const geometry = animationFrame();

        return {
          x: geometry.left + frame * geometry.width + (frame + 1) * geometry.border,
          y: geometry.top + animation.row * geometry.height + (animation.row + 1) * geometry.border,
        };
      }

      function animationFrame() {
        return animation.frame || sprite.frame;
      }

      function updateCanvasDimensions() {
        const frame = animationFrame();

        if (canvas.width !== frame.width || canvas.height !== frame.height) {
          canvas.width = frame.width;
          canvas.height = frame.height;
          context.imageSmoothingEnabled = false;
        }
      }

      return section;
    }

    function appendUnavailableMessage(section, text) {
      const message = document.createElement("div");
      message.className = "empty error";
      message.textContent = text;
      section.appendChild(message);
    }

    function animationFrames(animation) {
      const step = animation.to >= animation.from ? 1 : -1;
      const frames = [];

      for (let frame = animation.from; step > 0 ? frame <= animation.to : frame >= animation.to; frame += step) {
        frames.push(frame);
      }

      return frames;
    }
`;
}

module.exports = {
  spritePreviewRuntime,
};
