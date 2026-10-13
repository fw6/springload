import { Route, Routes } from 'react-router-dom'
import { useFlow } from './flow/useFlow'
import { PrototypeSwitcher, useVariant } from './variant'
import { variantA } from './variants/VariantA'
import { variantB } from './variants/VariantB'
import { variantC } from './variants/VariantC'

const modules = { A: variantA, B: variantB, C: variantC }

export default function App() {
  const variant = useVariant()
  const flow = useFlow()
  const mod = modules[variant]
  const Shell = mod.Shell
  return (
    <>
      <Shell flow={flow}>
        <Routes>
          <Route path="/" element={<mod.Flow flow={flow} />} />
          <Route path="/plan" element={<mod.Plan flow={flow} />} />
          <Route path="/positions" element={<mod.Positions />} />
          <Route path="/journal" element={<mod.Journal />} />
          <Route path="/settings" element={<mod.Settings />} />
        </Routes>
      </Shell>
      <PrototypeSwitcher />
    </>
  )
}
