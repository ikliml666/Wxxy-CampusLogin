// 设备形态判定(sw600dp 语义):短边 ≥ 600 CSS px 视为平板。
// viewport 为 width=device-width 时 1 CSS px ≈ 1dp,与 Android 原生
// Configuration.smallestScreenWidthDp >= 600 等价(react-native-device-info 同标准)。
// 用短边而非"宽>高":旋转不翻转结论——手机横屏宽>高但短边仍 <600 仍是手机,
// 平板竖屏宽<高但短边仍 ≥600 仍是平板(Material 3: medium 600-840 / expanded ≥840)。

import { useEffect, useState } from 'react'

export type FormFactor = 'phone' | 'tablet'
export type Orientation = 'portrait' | 'landscape'

/** Android 平板官方分界:smallest-width 600dp(sw600dp) */
const TABLET_MIN_EDGE_PX = 600

function detectFormFactor(): FormFactor {
  return Math.min(window.innerWidth, window.innerHeight) >= TABLET_MIN_EDGE_PX ? 'tablet' : 'phone'
}

function detectOrientation(): Orientation {
  return window.innerWidth >= window.innerHeight ? 'landscape' : 'portrait'
}

// 视口度量共用订阅:resize(旋转/分屏)与 orientationchange 时重算
function useViewportMeasure<T>(calc: () => T): T {
  const [value, setValue] = useState<T>(calc)
  useEffect(() => {
    const onChange = () => setValue(calc())
    window.addEventListener('resize', onChange)
    window.addEventListener('orientationchange', onChange)
    return () => {
      window.removeEventListener('resize', onChange)
      window.removeEventListener('orientationchange', onChange)
    }
  }, [])
  return value
}

export function useFormFactor(): FormFactor {
  return useViewportMeasure(detectFormFactor)
}

/** 当前方向(宽≥高为横屏),响应旋转/分屏实时切换 */
export function useOrientation(): Orientation {
  return useViewportMeasure(detectOrientation)
}
