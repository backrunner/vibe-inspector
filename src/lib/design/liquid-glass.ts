export interface LensMapOptions {
  width: number;
  height: number;
  radius: number;
  scale?: number;
  depth?: number;
  curvature?: number;
  opacity?: number;
  /** Neutral padding around the lens so rim displacement samples real painted pixels. */
  margin?: number;
}

export interface LensMap {
  dataUrl: string;
  scale: number;
  width: number;
  height: number;
}

const neutral = 128;

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value));
}

function roundedRectDistance(
  x: number,
  y: number,
  centerX: number,
  centerY: number,
  halfWidth: number,
  halfHeight: number,
  radius: number,
): number {
  const deltaX = Math.abs(x - centerX) - (halfWidth - radius);
  const deltaY = Math.abs(y - centerY) - (halfHeight - radius);
  const outside = Math.hypot(Math.max(0, deltaX), Math.max(0, deltaY));
  const inside = Math.min(Math.max(deltaX, deltaY), 0);
  return outside + inside - radius;
}

function writePixel(
  data: Uint8ClampedArray,
  width: number,
  x: number,
  y: number,
  red: number,
  green: number,
  alpha: number,
): void {
  const index = (y * width + x) * 4;
  data[index] = red;
  data[index + 1] = green;
  data[index + 2] = neutral;
  data[index + 3] = alpha;
}

export function generateLensMap({
  width,
  height,
  radius,
  scale = 0.09,
  depth = 9,
  curvature = 44,
  opacity = 0.88,
  margin = 0,
}: LensMapOptions): LensMap {
  const marginPx = Math.max(0, Math.round(margin));
  const mapWidth = Math.max(1, Math.round(width) + marginPx * 2);
  const mapHeight = Math.max(1, Math.round(height) + marginPx * 2);
  const canvas = document.createElement('canvas');
  canvas.width = mapWidth;
  canvas.height = mapHeight;

  const context = canvas.getContext('2d');
  if (!context) throw new Error('Unable to create a Liquid Glass displacement map');

  const image = context.createImageData(mapWidth, mapHeight);
  const centerX = mapWidth / 2;
  const centerY = mapHeight / 2;
  const halfWidth = Math.round(width) / 2;
  const halfHeight = Math.round(height) / 2;
  const lensRadius = Math.min(radius, halfWidth, halfHeight);
  const centerDistance = Math.abs(
    roundedRectDistance(centerX, centerY, centerX, centerY, halfWidth, halfHeight, lensRadius),
  );
  const lensOpacity = clamp(opacity, 0, 1);
  const halfMapWidth = Math.ceil(mapWidth / 2);
  const halfMapHeight = Math.ceil(mapHeight / 2);

  for (let y = 0; y < halfMapHeight; y += 1) {
    for (let x = 0; x < halfMapWidth; x += 1) {
      const distance = roundedRectDistance(
        x,
        y,
        centerX,
        centerY,
        halfWidth,
        halfHeight,
        lensRadius,
      );
      let red = neutral;
      let green = neutral;
      let alpha = 0;

      if (distance < 0) {
        const rimDistance = clamp(1 + distance / centerDistance, 0, 1);
        const magnitude = (depth / 10) * Math.pow(rimDistance, curvature / 10);
        const directionX = (x - centerX) / halfWidth;
        const directionY = (y - centerY) / halfHeight;
        const directionLength = Math.hypot(directionX, directionY) || 1;
        red = clamp(neutral + Math.round((magnitude * directionX * 127) / directionLength), 0, 255);
        green = clamp(
          neutral + Math.round((magnitude * directionY * 127) / directionLength),
          0,
          255,
        );
        alpha = Math.round(255 * lensOpacity);
      } else if (distance < 1) {
        alpha = Math.round((1 - distance) * 255 * lensOpacity);
      }

      const mirroredX = mapWidth - 1 - x;
      const mirroredY = mapHeight - 1 - y;
      writePixel(image.data, mapWidth, x, y, red, green, alpha);
      if (mirroredX !== x) {
        writePixel(image.data, mapWidth, mirroredX, y, neutral * 2 - red, green, alpha);
      }
      if (mirroredY !== y) {
        writePixel(image.data, mapWidth, x, mirroredY, red, neutral * 2 - green, alpha);
      }
      if (mirroredX !== x && mirroredY !== y) {
        writePixel(
          image.data,
          mapWidth,
          mirroredX,
          mirroredY,
          neutral * 2 - red,
          neutral * 2 - green,
          alpha,
        );
      }
    }
  }

  context.putImageData(image, 0, 0);
  return {
    dataUrl: canvas.toDataURL('image/png'),
    scale: scale * Math.round(width),
    width: mapWidth,
    height: mapHeight,
  };
}
