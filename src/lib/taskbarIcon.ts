const SIZE = 24;

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = reject;
    img.src = src;
  });
}

// Preload the base Dofus icon once for the lifetime of the app
let baseIconPromise: Promise<HTMLImageElement> | null = null;
function getBaseIcon(): Promise<HTMLImageElement> {
  if (!baseIconPromise) baseIconPromise = loadImage("/dofus-icon.png");
  return baseIconPromise;
}

/**
 * Renders a 24×24 taskbar icon composed of:
 *   1. Filled anti-aliased disc in `color` (if set)
 *   2. Dofus base icon at full size
 *   3. Class overlay icon at 16×16, bottom-right (if set), with 1px shadow
 *
 * Returns the raw RGBA pixel array (24 * 24 * 4 = 2304 values).
 */
export async function renderAccountIcon(
  iconPath: string | null,
  color: string | null
): Promise<number[]> {
  const canvas = document.createElement("canvas");
  canvas.width = SIZE;
  canvas.height = SIZE;
  const ctx = canvas.getContext("2d")!;

  // Layer 1: filled disc
  if (color) {
    ctx.fillStyle = color.startsWith("#") ? color : `#${color}`;
    ctx.beginPath();
    ctx.arc(SIZE / 2, SIZE / 2, SIZE / 2 - 1, 0, Math.PI * 2);
    ctx.fill();
  }

  // Layer 2: base Dofus icon
  const base = await getBaseIcon();
  ctx.drawImage(base, 0, 0, SIZE, SIZE);

  // Layer 3: class overlay with drop shadow
  if (iconPath) {
    try {
      const overlay = await loadImage(`/icons/${iconPath}.png`);
      // Shadow: semi-transparent offset copy
      ctx.globalAlpha = 0.4;
      ctx.drawImage(overlay, 9, 9, 16, 16);
      ctx.globalAlpha = 1.0;
      ctx.drawImage(overlay, 8, 8, 16, 16);
    } catch {
      // overlay load failed — skip silently
    }
  }

  return Array.from(ctx.getImageData(0, 0, SIZE, SIZE).data);
}
