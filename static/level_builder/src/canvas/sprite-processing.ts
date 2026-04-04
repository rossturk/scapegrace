// Sprite post-processing: remove black background, crop to content, center in square.

export function removeBlackBackground(b64: string): Promise<string> {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => {
      const c = document.createElement('canvas');
      c.width = img.width;
      c.height = img.height;
      const ctx = c.getContext('2d')!;
      ctx.drawImage(img, 0, 0);
      const data = ctx.getImageData(0, 0, c.width, c.height);
      const px = data.data;
      for (let i = 0; i < px.length; i += 4) {
        const r = px[i], g = px[i + 1], b = px[i + 2];
        const brightness = (r + g + b) / 3;
        if (brightness < 30) {
          px[i + 3] = 0; // near-black → fully transparent
        } else if (brightness < 60) {
          px[i + 3] = Math.round((brightness - 30) / 30 * 255); // fade edge
        }
      }
      ctx.putImageData(data, 0, 0);
      resolve(c.toDataURL('image/png').split(',')[1]);
    };
    img.onerror = () => resolve(b64);
    img.src = 'data:image/png;base64,' + b64;
  });
}

export function cropToContent(b64: string): Promise<string> {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => {
      const c = document.createElement('canvas');
      c.width = img.width;
      c.height = img.height;
      const ctx = c.getContext('2d')!;
      ctx.drawImage(img, 0, 0);
      const data = ctx.getImageData(0, 0, c.width, c.height);
      const px = data.data;
      let minX = c.width, minY = c.height, maxX = 0, maxY = 0;
      for (let y = 0; y < c.height; y++) {
        for (let x = 0; x < c.width; x++) {
          const a = px[(y * c.width + x) * 4 + 3];
          if (a > 10) {
            if (x < minX) minX = x;
            if (x > maxX) maxX = x;
            if (y < minY) minY = y;
            if (y > maxY) maxY = y;
          }
        }
      }
      if (maxX <= minX || maxY <= minY) { resolve(b64); return; }
      minX = Math.max(0, minX - 1);
      minY = Math.max(0, minY - 1);
      maxX = Math.min(c.width - 1, maxX + 1);
      maxY = Math.min(c.height - 1, maxY + 1);
      const cw = maxX - minX + 1, ch = maxY - minY + 1;
      const size = Math.max(cw, ch);
      const out = document.createElement('canvas');
      out.width = size;
      out.height = size;
      const octx = out.getContext('2d')!;
      octx.drawImage(c, minX, minY, cw, ch, (size - cw) / 2, (size - ch) / 2, cw, ch);
      resolve(out.toDataURL('image/png').split(',')[1]);
    };
    img.onerror = () => resolve(b64);
    img.src = 'data:image/png;base64,' + b64;
  });
}

export function downscale(b64: string, size: number = 16): Promise<string> {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => {
      const c = document.createElement('canvas');
      c.width = size;
      c.height = size;
      const ctx = c.getContext('2d')!;
      ctx.imageSmoothingEnabled = false;
      ctx.drawImage(img, 0, 0, size, size);
      resolve(c.toDataURL('image/png').split(',')[1]);
    };
    img.onerror = () => resolve(b64);
    img.src = 'data:image/png;base64,' + b64;
  });
}

export async function processSprite(rawB64: string, size: number = 16): Promise<string> {
  const transparent = await removeBlackBackground(rawB64);
  const cropped = await cropToContent(transparent);
  return downscale(cropped, size);
}

export function blendWithColor(b64: string, hexColor: string, strength: number): Promise<string> {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => {
      const c = document.createElement('canvas');
      c.width = img.width;
      c.height = img.height;
      const ctx = c.getContext('2d')!;
      ctx.drawImage(img, 0, 0);
      ctx.globalAlpha = strength;
      ctx.fillStyle = hexColor;
      ctx.fillRect(0, 0, c.width, c.height);
      const blended = c.toDataURL('image/png').split(',')[1];
      downscale(blended).then(resolve);
    };
    img.onerror = () => resolve(b64);
    img.src = 'data:image/png;base64,' + b64;
  });
}
