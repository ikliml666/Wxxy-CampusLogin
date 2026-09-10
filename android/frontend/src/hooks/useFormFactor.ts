// 设备形态判定(sw600dp 语义):短边 ≥ 600 CSS px 视为平板。
// viewport 为 width=device-width 时 1 CSS px ≈ 1dp,与 Android 原生
// Configuration.smallestScreenWidthDp >= 600 等价(react-native-device-info 同标准)。
// 用短边而非"宽>高":旋转不翻转结论——手机横屏宽>高但短边仍 <600 仍是手机,
// 平板竖屏宽<高但短边仍 ≥600 仍是平板(Material 3: medium 600-840 / expanded ≥840)。

import { useEffect, useState } from 'react'

export type FormFactor = 'phone' | 'tablet'

/** Android 平板官方分界:smallest-width 600dp(sw600dp) */
const TABLET_MIN_EDGE_PX = 600

function detectFormFactor(): FormFactor {
  return Math.min(window.innerWidth, window.innerHeight) >= TABLET_MIN_EDGE_PX ? 'tablet' : 'phone'
}

export function useFormFactor(): FormFactor {
  const [formFactor, setFormFactor] = useState<FormFactor>(detectFormFactor)
  useEffect(() => {
    // 旋转与分屏都会触发 resize,重算短边
    const onChange = () => setFormFactor(detectFormFactor())
    window.addEventListener('resize', onChange)
    window.addEventListener('orientationchange', onChange)
    return () => {
      window.removeEventListener('resize', onChange)
      window.removeEventListener('orientationchange', onChange)
    }
  }, [])
  return formFactor
}
