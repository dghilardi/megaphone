import { useState } from 'react'
import reactLogo from './assets/react.svg'
import { Message } from './components/Message';

const messages = [
  { text: 'Lorem ipsum dolor sit amet, consectetur adipiscing elit.', sent: true, ts: '2022-11-22' },
  { text: 'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod.', sent: true, ts: '2022-11-22' },
  { text: 'Lorem ipsum dolor sit amet.', sent: false, ts: '2022-11-22' },
  { text: 'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.', sent: true, ts: '2022-11-22' },
];

function App() {
  const [count, setCount] = useState(0)

  return (
    <div className="App flex flex-col items-center justify-center w-screen min-h-screen bg-gray-100 text-gray-800 p-10">
      <div className="flex flex-col flex-grow w-full max-w-xl bg-white shadow-xl rounded-lg overflow-hidden">
        <div className="flex flex-col flex-grow h-0 p-4 overflow-auto">
          {messages.map(msg => <Message direction={msg.sent?'sent':'received'} message={msg.text} ts={msg.ts}/>)}
        </div>

        <div className="bg-gray-300 p-4">
          <input className="flex items-center h-10 w-full rounded px-3 text-sm" type="text" placeholder="Type your message…" />
        </div>
      </div>
    </div>
  )
}

export default App
