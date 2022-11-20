#! /usr/bin/env -S deno run --allow-net

const baseUrl = 'http://localhost:3000'

interface ReaderCtx {
    terminate: boolean,
}

const spawnReader = async (channelId: string, ctx: ReaderCtx) => {
    while (!ctx.terminate) {
        console.log(`reading channel ${channelId}`);
        const result = await fetch(`${baseUrl}/read/${channelId}`)
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

const channelUuid = await fetch(`${baseUrl}/create`, { method: 'POST' })
    .then((resp) => {
        if (!resp.ok) {
            throw new Error("HTTP status code: " + resp.status);
        }
        return resp.text();
    });

const readerCtx = { terminate: false };
const readPromise = spawnReader(channelUuid, readerCtx);

const requests = Array.from(Array(150).keys())
    .map(index => ({
        index
    }));

for (const req of requests) {
    await new Promise( resolve => setTimeout(resolve, 100) );
    await fetch(`${baseUrl}/write/${channelUuid}/${req.index % 5}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(req),
    });
}

readerCtx.terminate = true;
await readPromise;

export { }