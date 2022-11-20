import { Suspense, useEffect, useState } from 'react'
import reactLogo from './assets/react.svg'
import { Message } from './components/Message';

interface Message {
  text: string,
  sent: boolean,
  ts: string,
}

interface ChatAppParams {
  room: string,
}

const spawnReader = async (channelId: string) => {
  while (true) {
      console.log(`reading channel ${channelId}`);
      const result = await fetch(`/read/${channelId}`)
          .then(async (resp) => {
              if (!resp.ok) {
                  throw new Error("HTTP status code: " + resp.status);
              }
              const reader = resp.body!
                  .pipeThrough(new TextDecoderStream())
                  .getReader();

              while (true) {
                  const { value, done } = await reader.read();
                  if (done) break;
                  console.log('Received', value);
              }
          });
  }
};

function ChatApp({ room }: ChatAppParams) {
  const [subscriptionId, setSubscriptionId] = useState<string>();
  const [messages, setMessages] = useState<Message[]>([]);

  useEffect(() => {
    fetch(`/room/${room}`, { method: 'POST' })
      .then(res => res.json())
      .then(res => setSubscriptionId(res.channelUuid))
  }, [ room ]);

  if (!subscriptionId) {
    return <p>Loading...</p>;
  }

  return <div className="flex flex-col flex-grow w-full max-w-xl bg-white shadow-xl rounded-lg overflow-hidden">
    <div className="flex flex-col flex-grow h-0 p-4 overflow-auto">
      {messages.map((msg, idx) => <Message key={idx} direction={msg.sent ? 'sent' : 'received'} message={msg.text} ts={msg.ts} />)}
    </div>

    <div className="bg-gray-300 p-4">
      <input className="flex items-center h-10 w-full rounded px-3 text-sm" type="text" placeholder="Type your message…" />
    </div>
  </div>;
}

function App() {
  return (
    <div className="App flex flex-col items-center justify-center w-screen min-h-screen bg-gray-100 text-gray-800 p-10">
      <ChatApp room="test" />
    </div>
  )
}

export default App
