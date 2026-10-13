import type { ReactElement, ReactNode } from 'react'
import type { Flow } from '../flow/useFlow'

export interface VariantModule {
  Shell: (props: { flow: Flow; children: ReactNode }) => ReactElement
  Flow: (props: { flow: Flow }) => ReactElement
  Plan: (props: { flow: Flow }) => ReactElement
  Positions: () => ReactElement
  Journal: () => ReactElement
  Settings: () => ReactElement
}
