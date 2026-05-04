export type SecurityPreset = 'none' | 'basic' | 'strict'

export interface TachyonConfig {
  security?: SecurityPreset
  /** Minimum body size in bytes to trigger gzip compression. 0 = compress all, -1 = disabled. Default: 1024 */
  compressionThreshold?: number
  /** Catch panics in handlers. Disable for max performance in controlled environments. Default: true */
  catchPanics?: boolean
}