import { createRoot } from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import App from './App'
import './styles.css'

const root = document.getElementById('root')
if (root === null) throw new Error('#root not found')
createRoot(root).render(
  <BrowserRouter>
    <App />
  </BrowserRouter>,
)
