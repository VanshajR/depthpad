// What is being dragged right now (dragover events cannot read the payload, so it lives here).
export const MIME = "application/x-depthpad";

export const drag = $state<{ target: string | null; from: number | null }>({ target: null, from: null });

export function startDrag(e: DragEvent, target: string, from: number | null, image?: Element | null) {
  drag.target = target;
  drag.from = from;
  e.dataTransfer?.setData(MIME, JSON.stringify({ target, from }));
  if (e.dataTransfer) e.dataTransfer.effectAllowed = "copyMove";
  if (image && e.dataTransfer) e.dataTransfer.setDragImage(image, 16, 16);
}

export function endDrag() {
  drag.target = null;
  drag.from = null;
}
