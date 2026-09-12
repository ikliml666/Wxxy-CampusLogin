/**
 * 应用内 2D 人脸验证（@vladmandic/human）。
 *
 * 场景：国产平板/手机的 2D 人脸多为 Class 1（便捷级），系统对三方应用完全
 * 不可达（BiometricPrompt 只暴露 Class 2/3），因此在"系统生物识别不可用 +
 * 用户开启 allow2dFaceVerify 且已录入"时，验证门回退到应用内人脸比对。
 *
 * 安全边界（用户已确认接受）：动作挑战（眨眼/转头）只能挡静态照片，
 * 不防重放视频；描述子模板仅存本机 safeStorage；比对通过后与生物链路共用
 * 同一后端 verify_biometric_identity TTL 门，后端不感知验证方式。
 *
 * 许可证：human 代码 MIT；BlazeFace/FaceMesh/HSE FaceRes 模型 Apache-2.0。
 * 不启用 human 的 antispoof/liveness/insightface 模型（许可不明或 NC），
 * 详见 THIRD-PARTY-NOTICES.md。
 */

import { Human } from '@vladmandic/human'
import { safeStorage } from '@/lib/utils'

const TEMPLATE_KEY = 'campus-2d-face-template'
/** human match.similarity 默认归一化注释："similarity above 0.5 can be considered a match"；
 * 从严取 0.55，真机标定后可调 */
const MATCH_THRESHOLD = 0.55
/** 录入采样帧数（质量门控通过帧的均值作为模板） */
const ENROLL_FRAMES = 8
/** 录入总时限 */
const ENROLL_TIMEOUT_MS = 20_000
/** 动作挑战时限 */
const CHALLENGE_TIMEOUT_MS = 10_000
/** 检测轮询间隔（转头是持续姿态不怕错过；眨眼靠帧间状态机记忆） */
const POLL_INTERVAL_MS = 120
/** 采帧质量门控：检测分下限 */
const DETECT_SCORE_MIN = 0.85

export type FaceChallenge = 'blink' | 'turn'

export type FaceFailReason = 'timeout' | 'challenge' | 'mismatch' | 'camera'
export type FaceVerifyResult = { ok: true } | { ok: false; reason: FaceFailReason }

let humanInstance: Human | null = null
let humanInitPromise: Promise<Human> | null = null

/** human 单例（懒加载 + warmup：WebGL 首帧 shader 编译秒级，须提前消化） */
async function getHuman(): Promise<Human> {
  if (humanInstance) return humanInstance
  if (!humanInitPromise) {
    humanInitPromise = (async () => {
      const h = new Human({
        backend: 'webgl',
        modelBasePath: '/models',
        face: {
          enabled: true,
          detector: { enabled: true, rotation: false },
          mesh: { enabled: true },
          attention: { enabled: false },
          iris: { enabled: false },
          description: { enabled: true },
          emotion: { enabled: false },
          // antispoof/liveness 模型许可不明，明确不启用（见 THIRD-PARTY-NOTICES.md）
          antispoof: { enabled: false },
          liveness: { enabled: false },
        },
        body: { enabled: false },
        hand: { enabled: false },
        object: { enabled: false },
        segmentation: { enabled: false },
      })
      await h.load()
      const warm = document.createElement('canvas')
      warm.width = 320
      warm.height = 320
      await h.detect(warm)
      humanInstance = h
      return h
    })()
  }
  return humanInitPromise
}

export async function openCamera(video: HTMLVideoElement): Promise<void> {
  const stream = await navigator.mediaDevices.getUserMedia({
    video: { facingMode: 'user', width: { ideal: 640 }, height: { ideal: 480 } },
    audio: false,
  })
  video.srcObject = stream
  await video.play()
}

export function closeCamera(video: HTMLVideoElement | null): void {
  const stream = video?.srcObject as MediaStream | null
  stream?.getTracks().forEach(t => t.stop())
  if (video) video.srcObject = null
}

interface FaceSnapshot {
  score: number
  embedding: number[] | null
  /** human 手势串（'facing left' / 'blink left eye' / ...），供转头判定 */
  gestures: string[]
  /** 本帧双眼是否闭合（human 同款 mesh 比率判据；眨眼是瞬态，靠状态机记忆） */
  eyesClosed: boolean
}

async function detectOnce(h: Human, video: HTMLVideoElement): Promise<FaceSnapshot | null> {
  const result = await h.detect(video)
  const face = result.face?.[0]
  if (!face || face.score < 0.5) return null
  const mesh = face.mesh as [number, number, number][] | undefined
  // 眨眼判据照抄 human src/gesture/gesture.ts 的 mesh 索引与比率（分母为同眼横向跨距）
  let eyesClosed = false
  if (mesh && mesh.length > 450) {
    const rightEye = Math.abs(mesh[374][1] - mesh[386][1]) / Math.abs(mesh[443][1] - mesh[450][1])
    const leftEye = Math.abs(mesh[145][1] - mesh[159][1]) / Math.abs(mesh[223][1] - mesh[230][1])
    eyesClosed = rightEye < 0.2 || leftEye < 0.2
  }
  return {
    score: face.score,
    embedding: face.embedding ?? null,
    gestures: (result.gesture ?? []).map(g => String(g.gesture)),
    eyesClosed,
  }
}

const sleep = (ms: number) => new Promise(r => setTimeout(r, ms))

// ---------------------------------------------------------------------------
// 模板存取（描述子均值，明文 localStorage——特征非原始人脸图像，且本机攻击者
// 本可直接调后端命令，加密不改变威胁模型）
// ---------------------------------------------------------------------------

export function loadTemplate(): number[] | null {
  const raw = safeStorage.get(TEMPLATE_KEY)
  if (!raw) return null
  try {
    const arr = JSON.parse(raw)
    return Array.isArray(arr) && arr.length > 0 ? arr : null
  } catch {
    return null
  }
}

export function hasTemplate(): boolean {
  return loadTemplate() !== null
}

export function clearTemplate(): void {
  safeStorage.set(TEMPLATE_KEY, '')
}

function saveTemplate(d: number[]): void {
  safeStorage.set(TEMPLATE_KEY, JSON.stringify(d))
}

/** 录入：质量门控采 ENROLL_FRAMES 帧描述子取均值；失败时相机由调用方负责关闭 */
export async function enrollFace(
  video: HTMLVideoElement,
  onProgress: (done: number, total: number) => void,
  onFacePresence?: (present: boolean) => void,
): Promise<{ ok: true } | { ok: false; reason: FaceFailReason }> {
  const h = await getHuman()
  const samples: number[][] = []
  const deadline = Date.now() + ENROLL_TIMEOUT_MS
  while (samples.length < ENROLL_FRAMES) {
    if (Date.now() > deadline) return { ok: false, reason: 'timeout' }
    const snap = await detectOnce(h, video).catch(() => null)
    onFacePresence?.(!!snap)
    // 门控：检测分够、有描述子、正脸（gesture 无明显转头）
    if (snap && snap.score >= DETECT_SCORE_MIN && snap.embedding
      && !snap.gestures.some(g => g === 'facing left' || g === 'facing right')) {
      samples.push(snap.embedding)
      onProgress(samples.length, ENROLL_FRAMES)
    }
    await sleep(POLL_INTERVAL_MS)
  }
  const dims = samples[0].length
  const mean = new Array<number>(dims).fill(0)
  for (const s of samples) for (let i = 0; i < dims; i++) mean[i] += s[i]
  for (let i = 0; i < dims; i++) mean[i] /= samples.length
  saveTemplate(mean)
  return { ok: true }
}

/** 验证：随机动作挑战 → 挑战通过后采帧与模板比对（3 次机会） */
export async function verifyFace(
  video: HTMLVideoElement,
  onChallenge: (c: FaceChallenge) => void,
  onFacePresence?: (present: boolean) => void,
): Promise<FaceVerifyResult> {
  const template = loadTemplate()
  if (!template) return { ok: false, reason: 'mismatch' }
  const h = await getHuman()

  const challenges: FaceChallenge[] = ['blink', 'turn']
  const challenge = challenges[Math.floor(Math.random() * challenges.length)]
  onChallenge(challenge)

  const deadline = Date.now() + CHALLENGE_TIMEOUT_MS
  let challengeDone = false
  while (Date.now() < deadline) {
    const snap = await detectOnce(h, video).catch(() => null)
    if (!snap) {
      onFacePresence?.(false)
      await sleep(POLL_INTERVAL_MS)
      continue
    }
    onFacePresence?.(true)
    if (challenge === 'blink') {
      // 瞬态动作：本帧闭合即记完成（窗口内一次有效眨眼）
      if (snap.eyesClosed) challengeDone = true
    } else if (snap.gestures.includes('facing left') || snap.gestures.includes('facing right')) {
      // 转头挑战：任一侧均可（镜像语义用户易转反，方向不是安全边界）
      challengeDone = true
    }
    if (challengeDone) break
    await sleep(POLL_INTERVAL_MS)
  }
  if (!challengeDone) return { ok: false, reason: 'challenge' }

  // 挑战通过后的同人比对：最多 3 帧，任一达到阈值即通过
  for (let attempt = 0; attempt < 3; attempt++) {
    const snap = await detectOnce(h, video).catch(() => null)
    if (snap?.embedding) {
      const sim = h.match.similarity(template, snap.embedding)
      if (sim >= MATCH_THRESHOLD) return { ok: true }
    }
    await sleep(250)
  }
  return { ok: false, reason: 'mismatch' }
}
