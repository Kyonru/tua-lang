"use strict";

// Self-contained so production webviews can inline this helper without an
// extra script resource or a second implementation of gesture math.
function createGraphViewport(scroll, content, options = {}) {
  const minimum = options.minimum ?? 0.25;
  const maximum = options.maximum ?? 2.5;
  const stage = document.createElement("div");
  stage.className = "graph-viewport-stage";
  content.before(stage);
  stage.append(content);
  content.style.transformOrigin = "0 0";
  content.style.minWidth = "0";
  content.style.minHeight = "0";
  let width = 1;
  let height = 1;
  let scale = Math.max(minimum, Math.min(maximum, options.initialScale || 1));
  const pointers = new Map();
  let pinch;

  function apply() {
    stage.style.width = width * scale + "px";
    stage.style.height = height * scale + "px";
    content.style.width = width + "px";
    content.style.height = height + "px";
    content.style.transform = "scale(" + scale + ")";
    options.onScale?.(scale);
  }

  function zoom(next, anchorX = scroll.clientWidth / 2, anchorY = scroll.clientHeight / 2) {
    const clamped = Math.max(minimum, Math.min(maximum, next));
    if (Math.abs(clamped - scale) < 0.001) return scale;
    const pointX = (scroll.scrollLeft + anchorX) / scale;
    const pointY = (scroll.scrollTop + anchorY) / scale;
    scale = clamped;
    apply();
    scroll.scrollLeft = pointX * scale - anchorX;
    scroll.scrollTop = pointY * scale - anchorY;
    return scale;
  }

  function fit() {
    return zoom(Math.min(1, scroll.clientWidth / width, scroll.clientHeight / height));
  }

  function point(event) {
    const rect = scroll.getBoundingClientRect();
    return { x: event.clientX - rect.left, y: event.clientY - rect.top };
  }

  function pinchSnapshot() {
    const [first, second] = [...pointers.values()];
    return {
      distance: Math.hypot(first.x - second.x, first.y - second.y),
      centerX: (first.x + second.x) / 2,
      centerY: (first.y + second.y) / 2,
      scale,
    };
  }

  scroll.addEventListener("wheel", (event) => {
    if (!event.ctrlKey && !event.metaKey) return;
    event.preventDefault();
    const anchor = point(event);
    zoom(scale * Math.exp(-event.deltaY * 0.01), anchor.x, anchor.y);
  }, { passive: false });

  scroll.addEventListener("pointerdown", (event) => {
    if (event.pointerType === "mouse" && event.button !== 0) return;
    if (event.target.closest("button, input, select, a, .edge")) return;
    scroll.setPointerCapture(event.pointerId);
    pointers.set(event.pointerId, point(event));
    scroll.classList.add("is-panning");
    if (pointers.size === 2) pinch = pinchSnapshot();
  });
  scroll.addEventListener("pointermove", (event) => {
    const previous = pointers.get(event.pointerId);
    if (!previous) return;
    const current = point(event);
    pointers.set(event.pointerId, current);
    if (pointers.size >= 2 && pinch) {
      const now = pinchSnapshot();
      zoom(pinch.scale * now.distance / Math.max(1, pinch.distance), now.centerX, now.centerY);
      scroll.scrollLeft -= now.centerX - pinch.centerX;
      scroll.scrollTop -= now.centerY - pinch.centerY;
      pinch = { ...now, scale };
    } else {
      scroll.scrollLeft -= current.x - previous.x;
      scroll.scrollTop -= current.y - previous.y;
    }
    event.preventDefault();
  });
  function endPointer(event) {
    pointers.delete(event.pointerId);
    if (pointers.size < 2) pinch = undefined;
    if (!pointers.size) scroll.classList.remove("is-panning");
  }
  scroll.addEventListener("pointerup", endPointer);
  scroll.addEventListener("pointercancel", endPointer);

  scroll.addEventListener("keydown", (event) => {
    if (event.target !== scroll) return;
    if (event.key === "+" || event.key === "=") zoom(scale * 1.2);
    else if (event.key === "-" || event.key === "_") zoom(scale / 1.2);
    else if (event.key === "0") zoom(1);
    else return;
    event.preventDefault();
  });

  apply();
  return {
    fit,
    scale: () => scale,
    setSize(nextWidth, nextHeight) {
      width = Math.max(1, nextWidth);
      height = Math.max(1, nextHeight);
      apply();
    },
    zoom,
  };
}

module.exports = { createGraphViewport };
