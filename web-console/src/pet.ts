import girlMint from "./pets/girl-mint.png";
import girlEmber from "./pets/girl-ember.png";
import girlNight from "./pets/girl-night.png";
import girlTide from "./pets/girl-tide.png";
import girlChip from "./pets/girl-chip.png";
import girlCotton from "./pets/girl-cotton.png";
import girlMintPoses from "./pets/girl-mint-poses.png";
import girlEmberPoses from "./pets/girl-ember-poses.png";
import girlNightPoses from "./pets/girl-night-poses.png";
import girlTidePoses from "./pets/girl-tide-poses.png";
import girlChipPoses from "./pets/girl-chip-poses.png";
import girlCottonPoses from "./pets/girl-cotton-poses.png";
import girlMintGen from "./pets/girl-mint-gen.png";
import girlEmberGen from "./pets/girl-ember-gen.png";
import girlNightGen from "./pets/girl-night-gen.png";
import girlTideGen from "./pets/girl-tide-gen.png";
import girlChipGen from "./pets/girl-chip-gen.png";
import girlCottonGen from "./pets/girl-cotton-gen.png";

export type ChatPetLifecycle = "idle" | "generating" | "running" | "waiting" | "review" | "failed";

type PetTrack =
  | "idle"
  | "running-right"
  | "running-left"
  | "waving"
  | "jumping"
  | "failed"
  | "waiting"
  | "running"
  | "review"
  | "generating";

type PetKind =
  | "sprout" | "fox" | "cat" | "slime" | "bot" | "bunny"
  | "mint" | "ember" | "night" | "tide" | "chip" | "cotton";
type PetStyle = "blob" | "girl";
type GirlHaircut = "twintails" | "short" | "long" | "bob" | "headset" | "bunny-hood";
type GirlAction = "idle" | "work" | "wait" | "review" | "fail" | "wave" | "gen";

interface GirlArt {
  idle: HTMLCanvasElement;
  work: HTMLCanvasElement;
  wait: HTMLCanvasElement;
  review: HTMLCanvasElement;
  fail: HTMLCanvasElement;
  wave: HTMLCanvasElement;
  gen: HTMLCanvasElement;
}

interface Pose {
  dx: number;
  dy: number;
  flip: boolean;
  rotate: number;
  blink: boolean;
  sad: boolean;
  glow: number;
  wave: number;
  lookX: number;
  squash: number;
  girl: GirlAction;
}

interface Track {
  row: number;
  frames: number;
  fps: number;
  loop: boolean;
}

interface PetPalette {
  ink: string;
  inkSad: string;
  bodyDeep: string;
  body: string;
  bodyLite: string;
  belly: string;
  ear: string;
  earIn: string;
  foot: string;
  accent: string;
  leaf: string;
  bud: string;
  eyeWhite: string;
  iris: string;
  shine: string;
  blush: string;
  nose: string;
  glow: string;
}

interface PetSpecies {
  id: PetKind;
  name: string;
  hint: string;
  style: PetStyle;
  haircut?: GirlHaircut;
  palette: PetPalette;
}

const ATLAS_COLS = 8;
const ATLAS_ROWS = 10;
const CELL_W = 192;
const CELL_H = 208;
const SSAA = 2;
const POSITION_KEY = "exoharness.chat-pet.pos";
const KIND_KEY = "exoharness.chat-pet.kind";
const ENABLED_KEY = "exoharness.chat-pet.enabled";
const DRAG_THRESHOLD = 4;

const TRACKS: Record<PetTrack, Track> = {
  idle: { row: 0, frames: 6, fps: 6, loop: true },
  "running-right": { row: 1, frames: 8, fps: 10, loop: true },
  "running-left": { row: 2, frames: 8, fps: 10, loop: true },
  waving: { row: 3, frames: 4, fps: 8, loop: false },
  jumping: { row: 4, frames: 5, fps: 8, loop: false },
  failed: { row: 5, frames: 8, fps: 8, loop: true },
  waiting: { row: 6, frames: 6, fps: 5, loop: true },
  running: { row: 7, frames: 6, fps: 8, loop: true },
  review: { row: 8, frames: 6, fps: 6, loop: true },
  generating: { row: 9, frames: 6, fps: 9, loop: true },
};

const STATUS_LABEL: Record<ChatPetLifecycle, string> = {
  idle: "",
  generating: "正在思考…",
  running: "正在执行…",
  waiting: "等待确认",
  review: "任务已完成",
  failed: "执行遇到问题",
};

const PETS: PetSpecies[] = [
  {
    id: "sprout",
    name: "芽芽",
    hint: "头顶小芽的绿团子",
    style: "blob",
    palette: {
      ink: "#16382c", inkSad: "#7a332e", bodyDeep: "#2f9a6c", body: "#5ad6a0",
      bodyLite: "#d2fbe8", belly: "#f4fff9", ear: "#3aa87a", earIn: "#f3c2b4",
      foot: "#2c7a58", accent: "#1f6b45", leaf: "#86eec4", bud: "#effff7",
      eyeWhite: "#f8fffb", iris: "#143028", shine: "#ffffff",
      blush: "rgba(232, 120, 110, .42)", nose: "#1b332b", glow: "rgba(155, 241, 204, .85)",
    },
  },
  {
    id: "fox",
    name: "焰焰",
    hint: "蓬尾小狐",
    style: "blob",
    palette: {
      ink: "#4a2418", inkSad: "#7a332e", bodyDeep: "#c45c28", body: "#f08a45",
      bodyLite: "#ffd2a8", belly: "#fff4ea", ear: "#c45c28", earIn: "#f3c2b4",
      foot: "#8a3a1c", accent: "#9a3d18", leaf: "#ffb072", bud: "#ffe7cc",
      eyeWhite: "#fffaf4", iris: "#3a1c12", shine: "#ffffff",
      blush: "rgba(232, 120, 110, .42)", nose: "#3a1c12", glow: "rgba(255, 176, 114, .8)",
    },
  },
  {
    id: "cat",
    name: "墨墨",
    hint: "夜色小猫",
    style: "blob",
    palette: {
      ink: "#1c1a2e", inkSad: "#7a332e", bodyDeep: "#3d3a5c", body: "#6b6790",
      bodyLite: "#c9c6e0", belly: "#f4f2ff", ear: "#3d3a5c", earIn: "#f3c2b4",
      foot: "#2a2740", accent: "#5b4d8a", leaf: "#b7a8e8", bud: "#efeaff",
      eyeWhite: "#f8f6ff", iris: "#f0c45c", shine: "#ffffff",
      blush: "rgba(232, 120, 110, .38)", nose: "#2a2740", glow: "rgba(183, 168, 232, .8)",
    },
  },
  {
    id: "slime",
    name: "波波",
    hint: "会发光的水滴",
    style: "blob",
    palette: {
      ink: "#163a52", inkSad: "#7a332e", bodyDeep: "#1d7eb0", body: "#4ec4e8",
      bodyLite: "#c8f3ff", belly: "#f3fcff", ear: "#3aa7c8", earIn: "#d7f6ff",
      foot: "#1a6a90", accent: "#1480b0", leaf: "#8be4ff", bud: "#e9fbff",
      eyeWhite: "#f5fdff", iris: "#123040", shine: "#ffffff",
      blush: "rgba(232, 120, 110, .35)", nose: "#163a52", glow: "rgba(120, 220, 255, .85)",
    },
  },
  {
    id: "bot",
    name: "核核",
    hint: "带天线的小机器人",
    style: "blob",
    palette: {
      ink: "#1a2a28", inkSad: "#7a332e", bodyDeep: "#2a4a44", body: "#7d8f8a",
      bodyLite: "#d5e4df", belly: "#9bf1cc", ear: "#4d635e", earIn: "#cfe0db",
      foot: "#243834", accent: "#77e5b5", leaf: "#9bf1cc", bud: "#e7fff4",
      eyeWhite: "#10211c", iris: "#77e5b5", shine: "#e7fff4",
      blush: "rgba(119, 229, 181, .2)", nose: "#1a2a28", glow: "rgba(119, 229, 181, .9)",
    },
  },
  {
    id: "bunny",
    name: "团团",
    hint: "垂耳小兔",
    style: "blob",
    palette: {
      ink: "#4a3228", inkSad: "#7a332e", bodyDeep: "#d4a07a", body: "#f3d7c0",
      bodyLite: "#fff1e6", belly: "#fff8f2", ear: "#f3d7c0", earIn: "#f4a8b8",
      foot: "#c48a68", accent: "#e8a0b0", leaf: "#f8c0cc", bud: "#ffe8ee",
      eyeWhite: "#fffaf6", iris: "#4a3228", shine: "#ffffff",
      blush: "rgba(232, 120, 110, .45)", nose: "#e07a8a", glow: "rgba(248, 192, 204, .8)",
    },
  },
  {
    id: "mint",
    name: "小芽",
    hint: "薄荷绿短发，元气校服",
    style: "girl",
    haircut: "twintails",
    palette: {
      ink: "#2a3d34", inkSad: "#7a332e", bodyDeep: "#3aa87a", body: "#7ee0b0",
      bodyLite: "#d9fbe8", belly: "#f6d4c2", ear: "#4ec89a", earIn: "#c8f5df",
      foot: "#2c7a58", accent: "#1f6b45", leaf: "#2f9a6c", bud: "#effff7",
      eyeWhite: "#fffaf7", iris: "#2a6b4a", shine: "#ffffff",
      blush: "rgba(232, 120, 110, .4)", nose: "#d4a090", glow: "rgba(155, 241, 204, .85)",
    },
  },
  {
    id: "ember",
    name: "小橙",
    hint: "橙发狐耳，活泼校服",
    style: "girl",
    haircut: "short",
    palette: {
      ink: "#4a2418", inkSad: "#7a332e", bodyDeep: "#c45c28", body: "#f08a45",
      bodyLite: "#ffd2a8", belly: "#f6d4c2", ear: "#e07a38", earIn: "#ffc48a",
      foot: "#8a3a1c", accent: "#9a3d18", leaf: "#c45c28", bud: "#ffe7cc",
      eyeWhite: "#fffaf7", iris: "#8a3a1c", shine: "#ffffff",
      blush: "rgba(232, 120, 110, .42)", nose: "#d4a090", glow: "rgba(255, 176, 114, .8)",
    },
  },
  {
    id: "night",
    name: "小夜",
    hint: "紫发猫耳，安静校服",
    style: "girl",
    haircut: "long",
    palette: {
      ink: "#1c1a2e", inkSad: "#7a332e", bodyDeep: "#3d3a5c", body: "#6b6790",
      bodyLite: "#c9c6e0", belly: "#f6d4c2", ear: "#4a466c", earIn: "#b7a8e8",
      foot: "#2a2740", accent: "#5b4d8a", leaf: "#2e2b44", bud: "#efeaff",
      eyeWhite: "#fffaf7", iris: "#c9a227", shine: "#ffffff",
      blush: "rgba(232, 120, 110, .38)", nose: "#d4a090", glow: "rgba(183, 168, 232, .8)",
    },
  },
  {
    id: "tide",
    name: "小汐",
    hint: "蓝发水滴，清爽校服",
    style: "girl",
    haircut: "bob",
    palette: {
      ink: "#163a52", inkSad: "#7a332e", bodyDeep: "#1d7eb0", body: "#4ec4e8",
      bodyLite: "#c8f3ff", belly: "#f6d4c2", ear: "#3aa7c8", earIn: "#b7ecff",
      foot: "#1a6a90", accent: "#1480b0", leaf: "#1d7eb0", bud: "#e9fbff",
      eyeWhite: "#fffaf7", iris: "#1a5a78", shine: "#ffffff",
      blush: "rgba(232, 120, 110, .35)", nose: "#d4a090", glow: "rgba(120, 220, 255, .85)",
    },
  },
  {
    id: "chip",
    name: "小芯",
    hint: "银发星标，科技校服",
    style: "girl",
    haircut: "headset",
    palette: {
      ink: "#1a2a28", inkSad: "#7a332e", bodyDeep: "#2a4a44", body: "#7d8f8a",
      bodyLite: "#d5e4df", belly: "#f6d4c2", ear: "#8aa09a", earIn: "#d5e4df",
      foot: "#243834", accent: "#77e5b5", leaf: "#5b706a", bud: "#e7fff4",
      eyeWhite: "#fffaf7", iris: "#1f6b4a", shine: "#ffffff",
      blush: "rgba(119, 229, 181, .25)", nose: "#d4a090", glow: "rgba(119, 229, 181, .9)",
    },
  },
  {
    id: "cotton",
    name: "小棉",
    hint: "粉发兔夹，甜美校服",
    style: "girl",
    haircut: "bunny-hood",
    palette: {
      ink: "#4a3228", inkSad: "#7a332e", bodyDeep: "#d4a07a", body: "#f4b8c4",
      bodyLite: "#ffe8ee", belly: "#f6d4c2", ear: "#f3d7c0", earIn: "#f4a8b8",
      foot: "#c48a68", accent: "#e8a0b0", leaf: "#e0c0aa", bud: "#fff8f2",
      eyeWhite: "#fffaf7", iris: "#8a4a58", shine: "#ffffff",
      blush: "rgba(232, 120, 110, .45)", nose: "#e07a8a", glow: "rgba(248, 192, 204, .8)",
    },
  },
];

const atlasCache = new Map<PetKind, string>();
const idleCache = new Map<PetKind, string>();
const girlSprites = new Map<PetKind, GirlArt>();
const gallerySprites: HTMLElement[] = [];
const GIRL_WAVE_SOURCES: Partial<Record<PetKind, string>> = {
  mint: girlMint,
  ember: girlEmber,
  night: girlNight,
  tide: girlTide,
  chip: girlChip,
  cotton: girlCotton,
};
const GIRL_POSE_SOURCES: Partial<Record<PetKind, string>> = {
  mint: girlMintPoses,
  ember: girlEmberPoses,
  night: girlNightPoses,
  tide: girlTidePoses,
  chip: girlChipPoses,
  cotton: girlCottonPoses,
};
const GIRL_GEN_SOURCES: Partial<Record<PetKind, string>> = {
  mint: girlMintGen,
  ember: girlEmberGen,
  night: girlNightGen,
  tide: girlTideGen,
  chip: girlChipGen,
  cotton: girlCottonGen,
};
let girlSpritesReady: Promise<void> | null = null;

let root: HTMLElement | null = null;
let sprite: HTMLElement | null = null;
let statusEl: HTMLElement | null = null;
let lifecycleMessage: string | null = null;
let currentKind: PetKind = "sprout";
let lifecycle: ChatPetLifecycle = "idle";
let overlay: PetTrack | null = null;
let frameIndex = 0;
let lastTick = 0;
let raf = 0;
let holdTimer = 0;
let idleJumpTimer = 0;
let dragging = false;
let customPosition = false;
let reduceMotion = false;
let galleryFrame = 0;
let galleryTimer = 0;
let motionMedia: MediaQueryList | null = null;
let onMotionChange: ((event: MediaQueryListEvent) => void) | null = null;
let onResize: (() => void) | null = null;

function petById(id: string | null | undefined): PetSpecies {
  return PETS.find((pet) => pet.id === id) ?? PETS[0];
}

function currentPet(): PetSpecies {
  return petById(currentKind);
}

function pose(partial: Partial<Pose> = {}): Pose {
  return {
    dx: 0, dy: 0, flip: false, rotate: 0, blink: false, sad: false,
    glow: 0, wave: 0, lookX: 0, squash: 1, girl: "idle", ...partial,
  };
}

function oval(ctx: CanvasRenderingContext2D, x: number, y: number, rx: number, ry: number): void {
  ctx.beginPath();
  ctx.ellipse(x, y, rx, ry, 0, 0, Math.PI * 2);
}

function fillStroke(
  ctx: CanvasRenderingContext2D,
  fill: string | CanvasGradient,
  stroke: string,
  width = 3.2,
): void {
  ctx.fillStyle = fill;
  ctx.strokeStyle = stroke;
  ctx.lineWidth = width;
  ctx.fill();
  ctx.stroke();
}

function bodyGradient(ctx: CanvasRenderingContext2D, pal: PetPalette): CanvasGradient {
  const body = ctx.createRadialGradient(-18, -10, 10, 4, 16, 74);
  body.addColorStop(0, pal.bodyLite);
  body.addColorStop(0.42, pal.body);
  body.addColorStop(1, pal.bodyDeep);
  return body;
}

function drawLeaf(
  ctx: CanvasRenderingContext2D,
  pal: PetPalette,
  ink: string,
  fromX: number,
  fromY: number,
  pullX: number,
  pullY: number,
  tipX: number,
  tipY: number,
): void {
  ctx.beginPath();
  ctx.moveTo(fromX, fromY);
  ctx.quadraticCurveTo(pullX, pullY, tipX, tipY);
  ctx.quadraticCurveTo(pullX * 0.35, fromY + 6, fromX, fromY + 8);
  ctx.closePath();
  fillStroke(ctx, pal.leaf, ink, 2.4);
}

function drawEye(
  ctx: CanvasRenderingContext2D,
  pal: PetPalette,
  x: number,
  y: number,
  poseState: Pose,
  ink: string,
  rx = 12.5,
  ry = 15,
): void {
  if (poseState.blink) {
    ctx.beginPath();
    ctx.moveTo(x - rx, y);
    ctx.quadraticCurveTo(x, y + 4, x + rx, y);
    ctx.strokeStyle = ink;
    ctx.lineWidth = 3.2;
    ctx.stroke();
    return;
  }
  oval(ctx, x, y, rx, ry);
  fillStroke(ctx, pal.eyeWhite, ink, 2.6);
  const look = poseState.lookX;
  const irisY = y + (poseState.sad ? 2.5 : 1);
  oval(ctx, x + look, irisY, rx * 0.58, ry * 0.6);
  ctx.fillStyle = poseState.sad ? pal.inkSad : pal.iris;
  ctx.fill();
  oval(ctx, x + look - 3.2, irisY - 3.4, 2.8, 3.4);
  ctx.fillStyle = pal.shine;
  ctx.fill();
  oval(ctx, x + look + 2.2, irisY + 3.2, 1.5, 1.8);
  ctx.fill();
}

function drawSparkles(ctx: CanvasRenderingContext2D, pal: PetPalette, poseState: Pose): void {
  if (poseState.girl !== "gen" || poseState.glow <= 0.15) return;
  ctx.save();
  ctx.fillStyle = pal.glow;
  ctx.shadowColor = pal.glow;
  ctx.shadowBlur = 8 + poseState.glow * 10;
  const phase = poseState.glow * Math.PI * 2;
  for (let i = 0; i < 3; i++) {
    const angle = phase + i * ((Math.PI * 2) / 3);
    oval(ctx, Math.cos(angle) * 46, Math.sin(angle) * 16 - 8, 3.4 + poseState.glow, 3.4 + poseState.glow);
    ctx.fill();
  }
  ctx.restore();
}

function drawGlowBud(ctx: CanvasRenderingContext2D, pal: PetPalette, ink: string, poseState: Pose, x: number, y: number, size: number): void {
  const bud = size + poseState.glow * 4;
  if (poseState.glow > 0) {
    ctx.save();
    ctx.shadowColor = pal.glow;
    ctx.shadowBlur = 10 + poseState.glow * 14;
  }
  oval(ctx, x, y, bud, bud);
  fillStroke(ctx, poseState.glow > 0.45 ? pal.bud : pal.leaf, ink, 2.4);
  if (poseState.glow > 0) ctx.restore();
}

function drawWaveArm(ctx: CanvasRenderingContext2D, pal: PetPalette, ink: string, poseState: Pose): void {
  if (poseState.wave <= 0) return;
  ctx.save();
  ctx.translate(54, 8);
  ctx.rotate(-0.35 - poseState.wave * 0.95);
  oval(ctx, 16, 0, 18, 10);
  fillStroke(ctx, pal.body, ink, 3);
  oval(ctx, 32, -3, 9, 9);
  fillStroke(ctx, pal.bodyLite, ink, 2.5);
  ctx.restore();
}

function drawFace(
  ctx: CanvasRenderingContext2D,
  pal: PetPalette,
  poseState: Pose,
  ink: string,
  options: { eyeY?: number; noseY?: number; mouthY?: number; visor?: boolean } = {},
): void {
  oval(ctx, -28, 18, 9, 5);
  ctx.fillStyle = pal.blush;
  ctx.fill();
  oval(ctx, 28, 18, 9, 5);
  ctx.fill();

  const eyeY = options.eyeY ?? 2;
  if (options.visor) {
    ctx.beginPath();
    ctx.roundRect(-30, eyeY - 12, 60, 26, 10);
    fillStroke(ctx, pal.eyeWhite, ink, 2.6);
    drawEye(ctx, pal, -14, eyeY, poseState, ink, 8, 8);
    drawEye(ctx, pal, 14, eyeY, poseState, ink, 8, 8);
  } else {
    drawEye(ctx, pal, -18, eyeY, poseState, ink);
    drawEye(ctx, pal, 18, eyeY, poseState, ink);
  }

  oval(ctx, 0, options.noseY ?? 16, 4.2, 3.2);
  ctx.fillStyle = pal.nose;
  ctx.fill();

  ctx.beginPath();
  if (poseState.sad) ctx.arc(0, (options.mouthY ?? 20) + 12, 9, Math.PI * 1.15, Math.PI * 1.85, true);
  else ctx.arc(0, options.mouthY ?? 20, 9, Math.PI * 0.18, Math.PI * 0.82);
  ctx.strokeStyle = ink;
  ctx.lineWidth = 2.8;
  ctx.stroke();
}

function drawRoundEars(ctx: CanvasRenderingContext2D, pal: PetPalette, ink: string): void {
  oval(ctx, -42, -38, 18, 26);
  fillStroke(ctx, pal.ear, ink, 3);
  oval(ctx, 42, -38, 18, 26);
  fillStroke(ctx, pal.ear, ink, 3);
  oval(ctx, -42, -36, 8, 12);
  ctx.fillStyle = pal.earIn;
  ctx.fill();
  oval(ctx, 42, -36, 8, 12);
  ctx.fill();
}

function drawPointedEars(ctx: CanvasRenderingContext2D, pal: PetPalette, ink: string, tall = false): void {
  const tip = tall ? -88 : -78;
  for (const sign of [-1, 1]) {
    ctx.beginPath();
    ctx.moveTo(sign * 12, -16);
    ctx.lineTo(sign * 36, tip);
    ctx.lineTo(sign * 48, -24);
    ctx.closePath();
    fillStroke(ctx, pal.ear, ink, 3);
    ctx.beginPath();
    ctx.moveTo(sign * 18, -24);
    ctx.lineTo(sign * 34, tip + 16);
    ctx.lineTo(sign * 40, -26);
    ctx.closePath();
    ctx.fillStyle = pal.earIn;
    ctx.fill();
  }
}

function drawBunnyEars(ctx: CanvasRenderingContext2D, pal: PetPalette, ink: string): void {
  for (const sign of [-1, 1]) {
    ctx.beginPath();
    ctx.ellipse(sign * 24, -72, 13, 38, sign * -0.22, 0, Math.PI * 2);
    fillStroke(ctx, pal.ear, ink, 3);
    ctx.beginPath();
    ctx.ellipse(sign * 24, -70, 5.5, 26, sign * -0.22, 0, Math.PI * 2);
    ctx.fillStyle = pal.earIn;
    ctx.fill();
  }
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("无法加载宠物立绘"));
    image.src = src;
  });
}

function knockoutPaper(image: CanvasImageSource, width: number, height: number): ImageData {
  const src = document.createElement("canvas");
  src.width = width;
  src.height = height;
  const ctx = src.getContext("2d");
  if (!ctx) throw new Error("Canvas 2D is unavailable");
  ctx.drawImage(image, 0, 0);
  const pixels = ctx.getImageData(0, 0, width, height);
  const { data } = pixels;
  const isPaper = (offset: number): boolean => {
    const r = data[offset];
    const g = data[offset + 1];
    const b = data[offset + 2];
    return r > 236 && g > 236 && b > 236 && Math.abs(r - g) < 14 && Math.abs(g - b) < 14;
  };
  const seen = new Uint8Array(width * height);
  const stack: number[] = [];
  const visit = (x: number, y: number): void => {
    if (x < 0 || y < 0 || x >= width || y >= height) return;
    const index = y * width + x;
    if (seen[index] || !isPaper(index * 4)) return;
    seen[index] = 1;
    stack.push(index);
  };
  for (let x = 0; x < width; x++) {
    visit(x, 0);
    visit(x, height - 1);
  }
  for (let y = 0; y < height; y++) {
    visit(0, y);
    visit(width - 1, y);
  }
  while (stack.length) {
    const index = stack.pop()!;
    data[index * 4 + 3] = 0;
    const x = index % width;
    const y = (index / width) | 0;
    visit(x - 1, y);
    visit(x + 1, y);
    visit(x, y - 1);
    visit(x, y + 1);
  }
  ctx.putImageData(pixels, 0, 0);
  return pixels;
}

function cropOpaque(source: CanvasImageSource, pixels: ImageData, bounds?: { minX: number; minY: number; maxX: number; maxY: number }): HTMLCanvasElement {
  const { data, width, height } = pixels;
  let minX = bounds?.minX ?? width;
  let minY = bounds?.minY ?? height;
  let maxX = bounds?.maxX ?? 0;
  let maxY = bounds?.maxY ?? 0;
  if (!bounds) {
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        if (data[(y * width + x) * 4 + 3] > 18) {
          minX = Math.min(minX, x);
          maxX = Math.max(maxX, x);
          minY = Math.min(minY, y);
          maxY = Math.max(maxY, y);
        }
      }
    }
  }
  const pad = 6;
  minX = Math.max(0, minX - pad);
  minY = Math.max(0, minY - pad);
  maxX = Math.min(width - 1, maxX + pad);
  maxY = Math.min(height - 1, maxY + pad);
  const out = document.createElement("canvas");
  out.width = Math.max(1, maxX - minX + 1);
  out.height = Math.max(1, maxY - minY + 1);
  const cut = out.getContext("2d");
  if (!cut) throw new Error("Canvas 2D is unavailable");
  cut.drawImage(source as CanvasImageSource, minX, minY, out.width, out.height, 0, 0, out.width, out.height);
  return out;
}

function prepareGirlSprite(image: HTMLImageElement): HTMLCanvasElement {
  const pixels = knockoutPaper(image, image.naturalWidth, image.naturalHeight);
  const src = document.createElement("canvas");
  src.width = pixels.width;
  src.height = pixels.height;
  src.getContext("2d")?.putImageData(pixels, 0, 0);
  return cropOpaque(src, pixels);
}

function extractGirlPoses(image: HTMLImageElement): HTMLCanvasElement[] {
  const pixels = knockoutPaper(image, image.naturalWidth, image.naturalHeight);
  const src = document.createElement("canvas");
  src.width = pixels.width;
  src.height = pixels.height;
  src.getContext("2d")?.putImageData(pixels, 0, 0);
  const { data, width, height } = pixels;
  const seen = new Uint8Array(width * height);
  const blobs: Array<{ minX: number; minY: number; maxX: number; maxY: number; area: number }> = [];
  const stack: number[] = [];
  for (let start = 0; start < width * height; start++) {
    if (seen[start] || data[start * 4 + 3] <= 18) continue;
    let minX = width;
    let minY = height;
    let maxX = 0;
    let maxY = 0;
    let area = 0;
    stack.push(start);
    seen[start] = 1;
    while (stack.length) {
      const index = stack.pop()!;
      const x = index % width;
      const y = (index / width) | 0;
      minX = Math.min(minX, x);
      maxX = Math.max(maxX, x);
      minY = Math.min(minY, y);
      maxY = Math.max(maxY, y);
      area += 1;
      for (const [nx, ny] of [[x - 1, y], [x + 1, y], [x, y - 1], [x, y + 1]] as const) {
        if (nx < 0 || ny < 0 || nx >= width || ny >= height) continue;
        const next = ny * width + nx;
        if (seen[next] || data[next * 4 + 3] <= 18) continue;
        seen[next] = 1;
        stack.push(next);
      }
    }
    if (area > 900 && maxY - minY > height * 0.35 && minX < width * 0.9) blobs.push({ minX, minY, maxX, maxY, area });
  }
  blobs.sort((a, b) => b.area - a.area);
  const chosen = blobs.slice(0, 5).sort((a, b) => a.minX - b.minX);
  if (chosen.length >= 5) return chosen.map((blob) => cropOpaque(src, pixels, blob));
  const poses: HTMLCanvasElement[] = [];
  const col = Math.floor(width / 5);
  for (let i = 0; i < 5; i++) {
    const slice = document.createElement("canvas");
    slice.width = col;
    slice.height = height;
    slice.getContext("2d")?.drawImage(src, i * col, 0, col, height, 0, 0, col, height);
    const cut = slice.getContext("2d")?.getImageData(0, 0, col, height);
    if (cut) poses.push(cropOpaque(slice, cut));
  }
  return poses;
}

function ensureGirlSprites(): Promise<void> {
  if (!girlSpritesReady) {
    girlSpritesReady = Promise.all(Object.keys(GIRL_WAVE_SOURCES).map(async (id) => {
      const kind = id as PetKind;
      const waveSrc = GIRL_WAVE_SOURCES[kind];
      const poseSrc = GIRL_POSE_SOURCES[kind];
      const genSrc = GIRL_GEN_SOURCES[kind];
      if (!waveSrc) return;
      const wave = prepareGirlSprite(await loadImage(waveSrc));
      const poses = poseSrc ? extractGirlPoses(await loadImage(poseSrc)) : [];
      const gen = genSrc ? prepareGirlSprite(await loadImage(genSrc)) : wave;
      girlSprites.set(kind, {
        idle: poses[0] ?? wave,
        work: poses[1] ?? wave,
        wait: poses[2] ?? wave,
        review: poses[3] ?? wave,
        fail: poses[4] ?? wave,
        wave,
        gen,
      });
    })).then(() => {
      atlasCache.clear();
      idleCache.clear();
    });
  }
  return girlSpritesReady;
}

function drawGirl(ctx: CanvasRenderingContext2D, poseState: Pose, species: PetSpecies): void {
  ctx.save();
  ctx.fillStyle = "rgba(8, 16, 14, .2)";
  ctx.beginPath();
  ctx.ellipse(0, 86 - poseState.dy * 0.12, 38, 8, 0, 0, Math.PI * 2);
  ctx.fill();
  const artSet = girlSprites.get(species.id);
  const art = artSet?.[poseState.girl] ?? artSet?.wave;
  if (!art) {
    ctx.restore();
    return;
  }
  const maxWidth = 152;
  const maxHeight = 176;
  const scale = Math.min(maxWidth / art.width, maxHeight / art.height);
  const width = art.width * scale;
  const height = art.height * scale;
  const x = -width / 2;
  const y = 88 - height;
  ctx.drawImage(art, x, y, width, height);
  // The source portraits already contain facial expressions. Painting guessed
  // eyelids over their faces made some variants look distorted; animate the
  // space around them instead of repainting their features.
  const activity = poseState.glow;
  if (activity > 0.1) {
    ctx.save();
    ctx.fillStyle = species.palette.glow;
    ctx.shadowColor = species.palette.glow;
    ctx.shadowBlur = 4 + activity * 9;
    const phase = poseState.wave * Math.PI * 2;
    for (let i = 0; i < 3; i++) {
      const angle = phase + i * Math.PI * 2 / 3;
      const px = Math.cos(angle) * (width * 0.48 + 5);
      const py = y + height * (0.25 + i * 0.17) + Math.sin(angle) * 5;
      const radius = 1.5 + activity * (i === 1 ? 2.2 : 1.3);
      ctx.beginPath();
      ctx.arc(px, py, radius, 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.restore();
  }
  ctx.restore();
}

function drawFigure(ctx: CanvasRenderingContext2D, poseState: Pose, species: PetSpecies): void {
  if (species.style === "girl") {
    drawGirl(ctx, poseState, species);
    return;
  }
  const pal = species.palette;
  const ink = poseState.sad ? pal.inkSad : pal.ink;
  ctx.lineJoin = "round";
  ctx.lineCap = "round";

  ctx.save();
  ctx.fillStyle = "rgba(8, 16, 14, .22)";
  ctx.beginPath();
  ctx.ellipse(0, 86 - poseState.dy * 0.12, 48, 9, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();

  if (species.id === "fox") {
    ctx.save();
    ctx.translate(-50, 28);
    ctx.rotate(-0.7);
    oval(ctx, 0, 0, 20, 36);
    fillStroke(ctx, pal.body, ink, 3);
    oval(ctx, 0, -22, 13, 16);
    fillStroke(ctx, pal.bodyLite, ink, 2.4);
    ctx.restore();
  } else if (species.id === "cat") {
    ctx.beginPath();
    ctx.moveTo(38, 28);
    ctx.quadraticCurveTo(78, 6, 58, -28);
    ctx.quadraticCurveTo(84, 18, 42, 38);
    ctx.closePath();
    fillStroke(ctx, pal.body, ink, 3);
  } else if (species.id === "bunny") {
    oval(ctx, -46, 36, 12, 10);
    fillStroke(ctx, pal.bodyLite, ink, 2.6);
  }

  if (species.id === "sprout") drawRoundEars(ctx, pal, ink);
  else if (species.id === "fox") drawPointedEars(ctx, pal, ink, false);
  else if (species.id === "cat") drawPointedEars(ctx, pal, ink, true);
  else if (species.id === "bunny") drawBunnyEars(ctx, pal, ink);

  if (species.id === "sprout") {
    ctx.beginPath();
    ctx.roundRect(-5, -86, 10, 34, 5);
    fillStroke(ctx, pal.accent, ink, 2.4);
    drawLeaf(ctx, pal, ink, -2, -72, -28, -94, -36, -76);
    drawLeaf(ctx, pal, ink, 2, -70, 30, -92, 38, -74);
    drawGlowBud(ctx, pal, ink, poseState, 0, -92, 9);
  } else if (species.id === "slime") {
    oval(ctx, 16, -68, 11, 18);
    fillStroke(ctx, pal.body, ink, 2.6);
    drawGlowBud(ctx, pal, ink, poseState, 16, -84, 8);
  } else if (species.id === "bot") {
    ctx.beginPath();
    ctx.roundRect(-3, -78, 6, 28, 3);
    fillStroke(ctx, pal.bodyDeep, ink, 2.4);
    drawGlowBud(ctx, pal, ink, poseState, 0, -86, 8);
  } else if (species.id === "fox" || species.id === "cat" || species.id === "bunny") {
    drawGlowBud(ctx, pal, ink, poseState, 0, -58, 6);
  }

  drawWaveArm(ctx, pal, ink, poseState);
  drawSparkles(ctx, pal, poseState);

  if (species.id === "bot") {
    ctx.beginPath();
    ctx.roundRect(-54, -22, 108, 92, 18);
    fillStroke(ctx, bodyGradient(ctx, pal), ink, 3.6);
    ctx.beginPath();
    ctx.roundRect(-22, 22, 44, 28, 8);
    ctx.fillStyle = pal.belly;
    ctx.fill();
    ctx.beginPath();
    ctx.roundRect(-28, 54, 20, 14, 5);
    fillStroke(ctx, pal.foot, ink, 2.6);
    ctx.beginPath();
    ctx.roundRect(8, 54, 20, 14, 5);
    fillStroke(ctx, pal.foot, ink, 2.6);
    drawFace(ctx, pal, poseState, ink, { eyeY: 0, noseY: 18, mouthY: 22, visor: true });
    return;
  }

  const bodyRx = species.id === "slime" ? 68 : 64;
  const bodyRy = species.id === "slime" ? 62 : 58;
  oval(ctx, 0, 10, bodyRx, bodyRy);
  fillStroke(ctx, bodyGradient(ctx, pal), ink, 3.6);
  oval(ctx, 0, 28, 34, 28);
  ctx.fillStyle = pal.belly;
  ctx.fill();
  oval(ctx, -24, 62, 16, 10);
  fillStroke(ctx, pal.foot, ink, 2.8);
  oval(ctx, 24, 62, 16, 10);
  fillStroke(ctx, pal.foot, ink, 2.8);
  if (species.id === "cat") {
    ctx.beginPath();
    ctx.moveTo(-40, 12);
    ctx.lineTo(-62, 8);
    ctx.moveTo(-40, 18);
    ctx.lineTo(-60, 22);
    ctx.moveTo(40, 12);
    ctx.lineTo(62, 8);
    ctx.moveTo(40, 18);
    ctx.lineTo(60, 22);
    ctx.strokeStyle = ink;
    ctx.lineWidth = 2;
    ctx.stroke();
  }
  drawFace(ctx, pal, poseState, ink);
}

function drawCanonical(poseState: Pose, species: PetSpecies): HTMLCanvasElement {
  const hi = document.createElement("canvas");
  hi.width = CELL_W * SSAA;
  hi.height = CELL_H * SSAA;
  const ctx = hi.getContext("2d");
  if (!ctx) throw new Error("Canvas 2D is unavailable");
  ctx.scale(SSAA, SSAA);
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  ctx.translate(CELL_W / 2 + poseState.dx, CELL_H / 2 + 8 + poseState.dy);
  if (species.style === "girl") ctx.translate(0, 72);
  ctx.rotate(poseState.rotate);
  if (species.style === "girl") ctx.translate(0, -72);
  ctx.scale(poseState.flip ? -poseState.squash : poseState.squash, 2 - poseState.squash);
  drawFigure(ctx, poseState, species);

  const cell = document.createElement("canvas");
  cell.width = CELL_W;
  cell.height = CELL_H;
  const out = cell.getContext("2d");
  if (!out) throw new Error("Canvas 2D is unavailable");
  out.imageSmoothingEnabled = true;
  out.imageSmoothingQuality = "high";
  out.drawImage(hi, 0, 0, CELL_W, CELL_H);
  return cell;
}

function girlTrackPoses(track: PetTrack): Pose[] {
  // These illustrations are expression poses, not a walk-cycle sprite sheet.
  // Keep a stable silhouette within a state and make motion pivot at the feet.
  switch (track) {
    case "idle":
      return [0, -1, -2, -2, -1, 0].map((dy, i) => pose({ girl: "idle", dy, rotate: [0, -0.008, -0.012, 0, 0.009, 0][i] }));
    case "running-right":
    case "running-left":
      return [0, -1, -2, -1, 0, -1, -2, -1].map((dy, i) => pose({
        girl: "wave", dy, flip: track === "running-left",
        rotate: (track === "running-left" ? -1 : 1) * [0, 0.008, 0.018, 0.008, 0, -0.008, -0.018, -0.008][i],
      }));
    case "waving":
      return [0, -0.025, -0.05, -0.02].map((rotate, i) => pose({ girl: "wave", rotate, dy: i === 2 ? -2 : 0, glow: i === 2 ? 0.55 : 0.2, wave: i / 4 }));
    case "jumping":
      return [0, -7, -17, -8, 0].map((dy, i) => pose({ girl: "wave", dy, rotate: [-0.01, -0.02, 0, 0.02, 0.01][i], glow: i === 2 ? 0.65 : 0.15, wave: i / 5 }));
    case "failed":
      return [0, 1, 2, 2, 1, 0, 1, 0].map((dy, i) => pose({ girl: "fail", dy, sad: true, rotate: [0, -0.008, -0.014, -0.018, -0.012, 0, 0.008, 0][i] }));
    case "waiting":
      return [0, -1, -1, 0, 1, 0].map((dy, i) => pose({ girl: "wait", dy, rotate: [0, -0.008, -0.012, 0, 0.008, 0][i], glow: i === 2 ? 0.25 : 0, wave: i / 6 }));
    case "running":
      return [0.2, 0.42, 0.7, 0.85, 0.55, 0.3].map((glow, i) => pose({ girl: "work", dy: i % 3 === 1 ? -1 : 0, glow, wave: i / 6 }));
    case "review":
      return [0.25, 0.48, 0.75, 0.9, 0.58, 0.3].map((glow, i) => pose({ girl: "review", dy: i === 2 || i === 3 ? -2 : 0, rotate: [0, -0.01, -0.015, 0, 0.015, 0][i], glow, wave: i / 6 }));
    case "generating":
      return [0.35, 0.6, 0.85, 1, 0.75, 0.45].map((glow, i) => pose({ girl: "gen", dy: i === 2 || i === 3 ? -2 : 0, glow, wave: i / 6 }));
  }
}

function trackPoses(track: PetTrack, species: PetSpecies): Pose[] {
  if (species.style === "girl") return girlTrackPoses(track);
  switch (track) {
    case "idle":
      return [
        pose({ girl: "idle" }),
        pose({ girl: "idle", dy: -4 }),
        pose({ girl: "idle", dy: -4, blink: true }),
        pose({ girl: "idle" }),
        pose({ girl: "idle", dy: 3 }),
        pose({ girl: "idle", glow: 0.35 }),
      ];
    case "running-right":
      return [8, 12, 8, 0, -4, 0, 8, 12].map((dx, i) => pose({
        girl: i % 2 === 0 ? "idle" : "work",
        dx, dy: i % 2 === 0 ? -4 : 0, rotate: 0.08,
      }));
    case "running-left":
      return [8, 12, 8, 0, -4, 0, 8, 12].map((dx, i) => pose({
        girl: i % 2 === 0 ? "idle" : "work",
        dx, dy: i % 2 === 0 ? -4 : 0, rotate: -0.08, flip: true,
      }));
    case "waving":
      return [
        pose({ girl: "idle", wave: 0.35, rotate: -0.05 }),
        pose({ girl: "wave", wave: 1, rotate: -0.1, glow: 0.5 }),
        pose({ girl: "wave", wave: 0.45, rotate: -0.04 }),
        pose({ girl: "idle", wave: 0.9, rotate: -0.08, glow: 0.35 }),
      ];
    case "jumping":
      return [
        pose({ girl: "idle", squash: 0.92 }),
        pose({ girl: "idle", dy: -18, squash: 1.05 }),
        pose({ girl: "wave", dy: -40, squash: 1.08, glow: 0.7 }),
        pose({ girl: "idle", dy: -18, squash: 1.04 }),
        pose({ girl: "idle", squash: 0.94 }),
      ];
    case "failed":
      return [-8, 8, -12, 12, -8, 4, 0, 0].map((dx, i) => pose({
        girl: "fail", dx, rotate: dx * 0.012, sad: true, blink: i === 3 || i === 7,
      }));
    case "waiting":
      return [
        pose({ girl: "wait", lookX: -8, rotate: -0.07 }),
        pose({ girl: "wait", lookX: -8, rotate: -0.07, blink: true }),
        pose({ girl: "wait" }),
        pose({ girl: "wait", lookX: 8, rotate: 0.07 }),
        pose({ girl: "wait", lookX: 8, rotate: 0.07 }),
        pose({ girl: "wait", blink: true }),
      ];
    case "running":
      return [0.35, 0.7, 1, 0.55, 0.9, 0.25].map((glow, i) => pose({
        girl: "work",
        dy: i % 2 === 0 ? -4 : -8, glow, rotate: i % 2 === 0 ? -0.03 : 0.03,
        blink: i === 2,
      }));
    case "review":
      return [
        pose({ girl: "review", rotate: -0.09, lookX: -6 }),
        pose({ girl: "review", rotate: -0.04 }),
        pose({ girl: "review", rotate: 0.09, lookX: 6 }),
        pose({ girl: "review", rotate: 0.04 }),
        pose({ girl: "review", rotate: -0.06, lookX: -6 }),
        pose({ girl: "review", blink: true }),
      ];
    case "generating":
      return [0.45, 0.75, 1, 0.62, 0.9, 0.35].map((glow, i) => pose({
        girl: "gen",
        dy: i % 2 === 0 ? -5 : -11,
        glow,
        rotate: i % 2 === 0 ? -0.04 : 0.04,
        wave: 0.25 + glow * 0.45,
      }));
  }
}

function buildAtlas(species: PetSpecies): string {
  const cached = atlasCache.get(species.id);
  if (cached) return cached;
  const atlas = document.createElement("canvas");
  atlas.width = ATLAS_COLS * CELL_W;
  atlas.height = ATLAS_ROWS * CELL_H;
  const ctx = atlas.getContext("2d");
  if (!ctx) throw new Error("Canvas 2D is unavailable");
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  (Object.keys(TRACKS) as PetTrack[]).forEach((name) => {
    const def = TRACKS[name];
    trackPoses(name, species).forEach((framePose, index) => {
      ctx.drawImage(drawCanonical(framePose, species), index * CELL_W, def.row * CELL_H);
    });
  });
  const url = atlas.toDataURL("image/png");
  atlasCache.set(species.id, url);
  return url;
}

function buildIdleSheet(species: PetSpecies): string {
  const cached = idleCache.get(species.id);
  if (cached) return cached;
  const sheet = document.createElement("canvas");
  sheet.width = 6 * CELL_W;
  sheet.height = CELL_H;
  const ctx = sheet.getContext("2d");
  if (!ctx) throw new Error("Canvas 2D is unavailable");
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  trackPoses("idle", species).forEach((framePose, index) => {
    ctx.drawImage(drawCanonical(framePose, species), index * CELL_W, 0);
  });
  const url = sheet.toDataURL("image/png");
  idleCache.set(species.id, url);
  return url;
}

function currentTrack(): PetTrack {
  if (overlay) return overlay;
  if (lifecycle === "generating") return "generating";
  if (lifecycle === "running") return "running";
  if (lifecycle === "waiting") return "waiting";
  if (lifecycle === "review") return "review";
  if (lifecycle === "failed") return "failed";
  return "idle";
}

function applyFrame(): void {
  if (!sprite || !root) return;
  const track = TRACKS[currentTrack()];
  const frame = reduceMotion ? 0 : Math.min(frameIndex, track.frames - 1);
  sprite.style.setProperty("--pet-row", String(track.row));
  sprite.style.setProperty("--pet-frame", String(frame));
  root.dataset.state = lifecycle;
  root.dataset.track = currentTrack();
  root.dataset.kind = currentKind;
  if (statusEl) {
    const text = lifecycleMessage ?? STATUS_LABEL[lifecycle];
    const changed = statusEl.textContent !== text;
    // Do not rewrite an aria-live region on every sprite animation frame.
    if (changed) statusEl.textContent = text;
    statusEl.hidden = !text;
    if (changed || dragging) root.dataset.bubbleSide = root.offsetTop < 76 ? "below" : "above";
  }
  const label = `对话宠物${lifecycleMessage || STATUS_LABEL[lifecycle] ? `，${lifecycleMessage || STATUS_LABEL[lifecycle]}` : "，等待新任务"}`;
  if (root.getAttribute("aria-label") !== label) root.setAttribute("aria-label", label);
}

function tick(now: number): void {
  raf = window.requestAnimationFrame(tick);
  if (reduceMotion) return;
  const track = TRACKS[currentTrack()];
  if (now - lastTick < 1000 / track.fps) return;
  lastTick = now;
  frameIndex += 1;
  if (frameIndex >= track.frames) {
    if (track.loop) frameIndex = 0;
    else {
      overlay = null;
      frameIndex = 0;
      scheduleIdleJump();
    }
  }
  applyFrame();
}

function playOverlay(track: PetTrack): void {
  overlay = track;
  frameIndex = 0;
  lastTick = 0;
  applyFrame();
}

function clearHold(): void {
  if (holdTimer) window.clearTimeout(holdTimer);
  holdTimer = 0;
}

function scheduleIdleJump(): void {
  if (idleJumpTimer) window.clearTimeout(idleJumpTimer);
  if (lifecycle !== "idle" || overlay || reduceMotion) return;
  idleJumpTimer = window.setTimeout(() => {
    if (lifecycle === "idle" && !overlay && !dragging) {
      playOverlay(Math.random() < 0.55 ? "waving" : "jumping");
    }
    scheduleIdleJump();
  }, 14000 + Math.random() * 9000);
}

function clampPosition(left: number, top: number): { left: number; top: number } {
  const workspace = root?.offsetParent as HTMLElement | null;
  const width = root?.offsetWidth ?? 124;
  const height = root?.offsetHeight ?? 170;
  const maxLeft = Math.max(8, (workspace?.clientWidth ?? window.innerWidth) - width - 8);
  const maxTop = Math.max(8, (workspace?.clientHeight ?? window.innerHeight) - height - 8);
  return {
    left: Math.min(maxLeft, Math.max(8, left)),
    top: Math.min(maxTop, Math.max(8, top)),
  };
}

function persistPosition(): void {
  if (!root || !customPosition) return;
  window.localStorage.setItem(POSITION_KEY, JSON.stringify({
    left: root.offsetLeft,
    top: root.offsetTop,
  }));
}

function restorePosition(): void {
  if (!root) return;
  try {
    const saved = JSON.parse(window.localStorage.getItem(POSITION_KEY) || "null") as { left?: number; top?: number } | null;
    if (!saved || typeof saved.left !== "number" || typeof saved.top !== "number") return;
    customPosition = true;
    const next = clampPosition(saved.left, saved.top);
    root.style.left = `${next.left}px`;
    root.style.top = `${next.top}px`;
    root.style.right = "auto";
    root.style.bottom = "auto";
  } catch {
    customPosition = false;
  }
}

function restoreKind(): PetKind {
  const saved = window.localStorage.getItem(KIND_KEY);
  return petById(saved).id;
}

/**
 * Pet display is a user preference.  Missing storage keeps the historical
 * behaviour (enabled), while an explicit false is respected on every chat
 * page load.
 */
export function isChatPetEnabled(): boolean {
  return window.localStorage.getItem(ENABLED_KEY) !== "false";
}

export function setChatPetEnabled(enabled: boolean): void {
  window.localStorage.setItem(ENABLED_KEY, String(enabled));
  if (enabled) {
    if (document.body.classList.contains("chat-page")) mountChatPet();
  } else {
    unmountChatPet();
  }
}

function selectPet(kind: PetKind): void {
  const species = petById(kind);
  currentKind = species.id;
  window.localStorage.setItem(KIND_KEY, species.id);
  if (sprite) sprite.style.backgroundImage = `url("${buildAtlas(species)}")`;
  root?.style.setProperty("--pet-accent", species.palette.glow);
  for (const spriteEl of gallerySprites) {
    const selected = spriteEl.dataset.kind === currentKind;
    spriteEl.classList.toggle("is-selected", selected);
    spriteEl.parentElement?.classList.toggle("is-selected", selected);
  }
  playOverlay("waving");
}

export function getChatPetKind(): string {
  return currentKind;
}

function petGroups(): Array<{ label: string; style: PetStyle; pets: PetSpecies[] }> {
  return [
    { label: "团子", style: "blob", pets: PETS.filter((pet) => pet.style === "blob") },
    { label: "小女孩", style: "girl", pets: PETS.filter((pet) => pet.style === "girl") },
  ];
}

function bindInteraction(): void {
  if (!root) return;
  let pointerId = 0;
  let startX = 0;
  let startY = 0;
  let originLeft = 0;
  let originTop = 0;
  let moved = false;

  root.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) return;
    pointerId = event.pointerId;
    startX = event.clientX;
    startY = event.clientY;
    originLeft = root?.offsetLeft ?? 0;
    originTop = root?.offsetTop ?? 0;
    moved = false;
    root?.setPointerCapture(pointerId);
  });

  root.addEventListener("pointermove", (event) => {
    if (!root || event.pointerId !== pointerId || !root.hasPointerCapture(pointerId)) return;
    const dx = event.clientX - startX;
    const dy = event.clientY - startY;
    if (!moved && Math.hypot(dx, dy) < DRAG_THRESHOLD) return;
    moved = true;
    dragging = true;
    customPosition = true;
    root.classList.add("is-dragging");
    overlay = dx < 0 ? "running-left" : "running-right";
    frameIndex = 0;
    const next = clampPosition(originLeft + dx, originTop + dy);
    root.style.left = `${next.left}px`;
    root.style.top = `${next.top}px`;
    root.style.right = "auto";
    root.style.bottom = "auto";
    applyFrame();
  });

  const endDrag = (event: PointerEvent) => {
    if (!root || event.pointerId !== pointerId) return;
    if (root.hasPointerCapture(pointerId)) root.releasePointerCapture(pointerId);
    pointerId = 0;
    root.classList.remove("is-dragging");
    if (moved) {
      dragging = false;
      overlay = null;
      frameIndex = 0;
      persistPosition();
      applyFrame();
      return;
    }
    dragging = false;
    playOverlay("waving");
  };

  root.addEventListener("pointerup", endDrag);
  root.addEventListener("pointercancel", endDrag);
  root.addEventListener("keydown", (event) => {
    if (event.target !== root) return;
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      playOverlay("waving");
    }
  });
  onResize = () => {
    if (!root || !customPosition) return;
    const next = clampPosition(root.offsetLeft, root.offsetTop);
    root.style.left = `${next.left}px`;
    root.style.top = `${next.top}px`;
  };
  window.addEventListener("resize", onResize);
}

export function setChatPetLifecycle(next: ChatPetLifecycle, options?: { persist?: boolean; message?: string }): void {
  if (!root) return;
  const hold = next === "review" || next === "failed";
  const message = options?.message ?? null;
  if (lifecycle === next && lifecycleMessage === message && !hold) return;
  clearHold();
  lifecycle = next;
  lifecycleMessage = message;
  if (overlay !== "waving" && overlay !== "jumping") overlay = null;
  frameIndex = 0;
  applyFrame();
  if (next === "idle") scheduleIdleJump();
  else if (idleJumpTimer) {
    window.clearTimeout(idleJumpTimer);
    idleJumpTimer = 0;
  }
  if (options?.persist) return;
  if (next === "review") {
    holdTimer = window.setTimeout(() => {
      if (lifecycle === "review") setChatPetLifecycle("idle");
    }, 5200);
  } else if (next === "failed") {
    holdTimer = window.setTimeout(() => {
      if (lifecycle === "failed") setChatPetLifecycle("idle");
    }, 6400);
  } else if (next === "idle" && message) {
    holdTimer = window.setTimeout(() => {
      if (lifecycle === "idle") setChatPetLifecycle("idle");
    }, 4200);
  }
}

export function syncChatPetFromEvent(event: ChatStreamEventLike): void {
  switch (event.type) {
    case "complete":
      setChatPetLifecycle("review");
      break;
    case "error":
    case "failed":
      setChatPetLifecycle("failed");
      break;
    case "cancelled":
      setChatPetLifecycle("idle", { message: "任务已停止" });
      break;
    case "recoverable":
      setChatPetLifecycle("waiting", { message: "任务已暂停，可继续执行" });
      break;
    case "resource_queue":
      setChatPetLifecycle(event.state === "waiting" ? "waiting" : "generating", {
        message: event.state === "waiting" ? "正在排队，等待执行资源" : "正在准备执行…",
      });
      break;
    case "phase":
      setChatPetLifecycle(event.phase === "waiting" || event.phase === "rate_limited" ? "waiting" : "generating", {
        message: event.phase === "rate_limited" ? "服务繁忙，等待重试" : event.phase === "waiting" ? "正在等待任务进展" : "正在思考…",
      });
      break;
    case "status":
      setChatPetLifecycle(event.status === "waiting_authorization" ? "waiting" : "generating", {
        message: event.status === "waiting_authorization" ? "等待你的授权" : event.status === "queued" ? "任务已排队" : "正在思考…",
      });
      break;
    case "run_created":
    case "assistant_start":
    case "turn_end":
      setChatPetLifecycle("generating");
      break;
    case "text_delta":
      setChatPetLifecycle("generating", { message: "正在深度思考…" });
      break;
    case "upstream_error":
      setChatPetLifecycle("waiting", { message: "服务暂时异常，等待重试" });
      break;
    case "tool_call":
    case "tool_output":
    case "tool_progress":
    case "artifact":
    case "tool_result":
      setChatPetLifecycle("running");
      break;
    default:
      break;
  }
}

export interface ChatStreamEventLike {
  type: string;
  state?: "waiting" | "acquired";
  phase?: string;
  status?: string;
}

export function mountChatPetGallery(host: HTMLElement): void {
  void ensureGirlSprites().then(() => {
    if (!host.isConnected) return;
    if (galleryTimer) window.clearInterval(galleryTimer);
    host.replaceChildren();
    host.classList.add("chat-pet-gallery");
    gallerySprites.length = 0;
    currentKind = restoreKind();
  for (const group of petGroups()) {
    const section = document.createElement("section");
    section.className = `chat-pet-gallery-group is-${group.style}`;
    const heading = document.createElement("h2");
    heading.textContent = group.label;
    const row = document.createElement("div");
    row.className = "chat-pet-gallery-row";
    for (const species of group.pets) {
      const card = document.createElement("button");
      card.type = "button";
      card.className = "chat-pet-card";
      card.dataset.kind = species.id;
      card.classList.toggle("is-selected", species.id === currentKind);
      const figure = document.createElement("span");
      figure.className = "chat-pet-card-sprite";
      figure.style.backgroundImage = `url("${buildIdleSheet(species)}")`;
      const name = document.createElement("strong");
      name.textContent = species.name;
      const hint = document.createElement("small");
      hint.textContent = species.hint;
      card.append(figure, name, hint);
      card.addEventListener("click", () => selectPet(species.id));
      row.append(card);
      gallerySprites.push(figure);
      figure.dataset.kind = species.id;
    }
    section.append(heading, row);
    host.append(section);
  }
  galleryTimer = window.setInterval(() => {
    if (!host.isConnected) {
      window.clearInterval(galleryTimer);
      galleryTimer = 0;
      return;
    }
    if (!host.getClientRects().length || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    galleryFrame = (galleryFrame + 1) % 6;
    for (const spriteEl of gallerySprites) {
      spriteEl.style.setProperty("--pet-frame", String(galleryFrame));
    }
  }, 160);
  });
}

/** Remove the live chat pet and stop its timers/animation before remounting. */
export function unmountChatPet(): void {
  if (raf) window.cancelAnimationFrame(raf);
  raf = 0;
  if (onResize) window.removeEventListener("resize", onResize);
  onResize = null;
  if (motionMedia && onMotionChange) motionMedia.removeEventListener("change", onMotionChange);
  motionMedia = null;
  onMotionChange = null;
  clearHold();
  if (idleJumpTimer) window.clearTimeout(idleJumpTimer);
  idleJumpTimer = 0;
  root?.remove();
  root = null;
  sprite = null;
  statusEl = null;
  overlay = null;
  frameIndex = 0;
  dragging = false;
}

export function mountChatPet(): void {
  if (root || !document.body.classList.contains("chat-page") || !isChatPetEnabled()) return;
  const workspace = document.querySelector<HTMLElement>("#chat-drop-zone");
  if (!workspace) return;

  motionMedia = window.matchMedia("(prefers-reduced-motion: reduce)");
  reduceMotion = motionMedia.matches;
  currentKind = restoreKind();

  root = document.createElement("div");
  root.className = "chat-pet";
  root.id = "chat-pet";
  root.tabIndex = 0;
  root.title = "点击打招呼，拖动可挪位置；在设置中更换宠物";
  root.style.setProperty("--pet-accent", currentPet().palette.glow);

  sprite = document.createElement("span");
  sprite.className = "chat-pet-sprite";
  sprite.setAttribute("aria-hidden", "true");

  statusEl = document.createElement("span");
  statusEl.className = "chat-pet-status";
  statusEl.setAttribute("role", "status");
  statusEl.setAttribute("aria-live", "polite");
  statusEl.setAttribute("aria-atomic", "true");

  root.append(sprite, statusEl);
  workspace.append(root);
  restorePosition();
  bindInteraction();
  applyFrame();
  void ensureGirlSprites().then(() => {
    if (!sprite) return;
    sprite.style.backgroundImage = `url("${buildAtlas(currentPet())}")`;
  });
  raf = window.requestAnimationFrame(tick);
  scheduleIdleJump();
  onMotionChange = (event) => {
    reduceMotion = event.matches;
    frameIndex = 0;
    applyFrame();
  };
  motionMedia.addEventListener("change", onMotionChange);
}
