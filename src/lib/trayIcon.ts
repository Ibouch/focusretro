const SIZE = 32;
// Scale up the app icon to clip its transparent padding. Increase if padding is still visible.
const ICON_SCALE = 1.25;

let appIconPromise: Promise<HTMLImageElement> | null = null;
function getAppIcon(): Promise<HTMLImageElement> {
  if (!appIconPromise) {
    appIconPromise = new Promise((resolve, reject) => {
      const img = new Image();
      img.onload = () => resolve(img);
      img.onerror = reject;
      img.src = "/app-icon.png";
    });
  }
  return appIconPromise;
}

/**
 * Renders a 32x32 tray icon: app icon with a green/red status dot
 * (dark border) in the bottom-right corner.
 * Returns the raw RGBA pixel array (32 * 32 * 4 = 4096 values).
 */
export async function renderTrayIcon(isActive: boolean): Promise<number[]> {
  const canvas = document.createElement("canvas");
  canvas.width = SIZE;
  canvas.height = SIZE;
  const ctx = canvas.getContext("2d")!;

  // Layer 1: app icon — scaled up to clip transparent padding
  const iconSize = SIZE * ICON_SCALE;
  const iconOffset = -(iconSize - SIZE) / 2;
  const icon = await getAppIcon();
  ctx.drawImage(icon, iconOffset, iconOffset, iconSize, iconSize);

  // Layer 2: dark border ring
  const dotX = SIZE - 5;
  const dotY = SIZE - 5;
  ctx.fillStyle = "rgba(0, 0, 0, 0.65)";
  ctx.beginPath();
  ctx.arc(dotX, dotY, 4.5, 0, Math.PI * 2);
  ctx.fill();

  // Layer 3: status dot
  ctx.fillStyle = isActive ? "#22c55e" : "#ef4444";
  ctx.beginPath();
  ctx.arc(dotX, dotY, 3, 0, Math.PI * 2);
  ctx.fill();

  return Array.from(ctx.getImageData(0, 0, SIZE, SIZE).data);
}
