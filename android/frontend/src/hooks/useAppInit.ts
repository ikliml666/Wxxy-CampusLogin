import { useEventListeners } from './useEventListeners'
import { useInitialDataLoad } from './useInitialDataLoad'
import { useHeartbeat } from './useHeartbeat'
import { useGlobalShortcut } from './useGlobalShortcut'

export function useAppInit() {
  useEventListeners()
  useInitialDataLoad()
  useHeartbeat()
  useGlobalShortcut()
}
