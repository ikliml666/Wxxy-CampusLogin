/**
 * 悬浮栏镜头系统的物理内核。
 *
 * FlClash 的悬浮栏（navigation_dock.dart 的 `_Spring`）用自建弹簧仿真器驱动
 * 十几条运动曲线：镜头位置、抬起、果冻形变、悬停高亮、整条膨胀、越界拉伸、
 * 光斑强度与扩散。
 *
 * 为什么非要弹簧而不是 CSS transition：拖动时目标每帧都在被改写，transition
 * 每次改目标都会从零速重新缓动，读起来一路顿挫；弹簧保留速度与位置连续，
 * 手指走到哪镜头跟到哪，还顺带给出惯性。FlClash 的注释里也记着同一件事——
 * `AnimationController.animateWith` 会重启 ticker，而 ticker 首帧读不到
 * 已过时间，于是「手指一直在动」时值几乎不动。
 *
 * 这里是同语义的最小实现：半隐式欧拉积分 + 共享 rAF，全部静止后自动停表。
 */

export interface SpringSpec {
  /** 刚度，越大越快 */
  stiffness: number
  /** 阻尼，临界阻尼为 2*sqrt(stiffness)，低于它会回弹 */
  damping: number
}

/**
 * 由「稳定时长 + 回弹量」反解弹簧参数。
 *
 * zeta = 1 - bounce（bounce=0 即临界阻尼）；稳定时间约 4/(zeta*wn)，
 * 于是 wn = 4/(zeta*duration)，stiffness = wn^2、damping = 2*zeta*wn。
 *
 * 与 Flutter `SpringDescription.withDurationAndBounce` 观感一致但非同一公式：
 * 后者只能照抄 Flutter 源码，而 FlClash 是 GPL-3.0，代码不可带入本项目。
 */
export function springOfDuration(durationMs: number, bounce = 0): SpringSpec {
  const zeta = Math.min(1, Math.max(0.05, 1 - bounce))
  const wn = 4 / (zeta * (durationMs / 1000))
  return { stiffness: wn * wn, damping: 2 * zeta * wn }
}

/** 由「刚度 + 阻尼比」构造（Flutter `withDampingRatio` 的对应物） */
export function springOfRatio(stiffness: number, ratio: number): SpringSpec {
  return { stiffness, damping: ratio * 2 * Math.sqrt(stiffness) }
}

/** 停表阈值：位置差与速度都小到看不出来 */
const REST_VALUE = 0.0015
const REST_VELOCITY = 0.02

/** 掉帧时（切窗口回来、长任务阻塞）钳住步长，否则一次大步会冲过目标乱跳 */
const MAX_STEP = 1 / 30

type FrameFn = (value: number) => void

const active = new Set<Motion>()
let rafHandle = 0
let lastTime = 0

function loop(now: number) {
  const dt = Math.min((now - lastTime) / 1000, MAX_STEP)
  lastTime = now
  // 快照遍历：某条曲线的回调可能拉起别的曲线（果冻由镜头速度驱动）
  for (const motion of [...active]) motion.step(dt)
  rafHandle = active.size > 0 ? requestAnimationFrame(loop) : 0
}

function schedule() {
  if (rafHandle) return
  lastTime = performance.now()
  rafHandle = requestAnimationFrame(loop)
}

/**
 * 一条标量运动曲线。`onFrame` 每帧收到当前值；值到位后自动停表。
 *
 * 与 `gsap.quickTo` 的区别：quickTo 改目标会重启补间（速度归零），这里保留
 * 速度——拖动镜头时那点惯性正是果冻感的来源。
 */
export class Motion {
  value: number
  private target: number
  private velocity = 0
  private spec: SpringSpec = springOfDuration(300)
  private onFrame: FrameFn

  constructor(initial: number, onFrame: FrameFn) {
    this.value = initial
    this.target = initial
    this.onFrame = onFrame
  }

  get current() {
    return this.value
  }

  get goal() {
    return this.target
  }

  /** 速度（单位/秒），果冻形变读它决定拉伸 */
  get speed() {
    return this.velocity
  }

  /** 加速度，果冻形变读它判断「正在刹车」 */
  get accel() {
    return this.spec.stiffness * (this.target - this.value) - this.spec.damping * this.velocity
  }

  /** 是否还在运动 */
  get moving() {
    return active.has(this)
  }

  /** 追向新目标。已在追同一目标时不重启（否则速度被清零）。 */
  to(target: number, spec?: SpringSpec) {
    if (spec) this.spec = spec
    if (target === this.target && active.has(this)) return
    this.target = target
    active.add(this)
    schedule()
  }

  /** 立即落位，不产生运动（初始化与「跳过动画」用） */
  jump(value: number) {
    this.value = value
    this.target = value
    this.velocity = 0
    active.delete(this)
    this.onFrame(value)
  }

  step(dt: number) {
    const { stiffness, damping } = this.spec
    this.velocity += (stiffness * (this.target - this.value) - damping * this.velocity) * dt
    this.value += this.velocity * dt

    if (Math.abs(this.target - this.value) < REST_VALUE && Math.abs(this.velocity) < REST_VELOCITY) {
      this.value = this.target
      this.velocity = 0
      active.delete(this)
    }
    this.onFrame(this.value)
  }

  dispose() {
    active.delete(this)
  }
}

/**
 * 橡皮筋：拖过边界后按指数衰减抵住。
 *
 * pull = 1 - 1/(|overshoot|*0.55/limit + 1)，越拖越多地「抵住」，永远拖不出
 * limit——比线性阻尼更接近 iOS。FlClash 用的是同一条式子。
 */
export function rubberBand(overshoot: number, limit: number): number {
  if (limit <= 0) return 0
  const pull = 1 - 1 / ((Math.abs(overshoot) * 0.55) / limit + 1)
  return limit * pull * Math.sign(overshoot)
}

/** tanh 型饱和：小值近似线性，大值压到 1 以内 */
export function saturate(value: number): number {
  return 1 - 2 / (Math.exp(2 * value) + 1)
}
