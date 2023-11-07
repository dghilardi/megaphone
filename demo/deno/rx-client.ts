#! /usr/bin/env -S deno run --allow-net

import { Observable, Subscriber } from 'npm:rxjs@7.6.0';
import { firstValueFrom } from "npm:rxjs@7.6.0";

type Chunk<T> = {
    sid: string;
    eid: string;
    ts: string;
    body: T;
};

class MegaphonePoller {
    private channelId?: string;
    private streams: Array<{ stream: string, subscriber: Subscriber<unknown> }> = [];
    constructor(
        private baseUrl: string,
    ) { }

    async spawnReader(channelId: string): Promise<void> {
        this.channelId = channelId;
        try {
            while (this.streams.length > 0) {
                await fetch(`${this.baseUrl}/read/${this.channelId}`)
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
                            value
                                .trim()
                                .split('\n')
                                .forEach(chunk => {
                                    const msg = JSON.parse(chunk);
                                    const stream = this.streams.find(({ stream }) => stream === msg.sid);
                                    if (stream) {
                                        stream.subscriber.next(msg);
                                    }
                                });
                        }
                    });
            }
        } finally {
            this.channelId = undefined;
        }
    }

    async newStream<T>(factory: (channelId?: string) => Promise<{ channelId: string, streamId: string }>): Promise<Observable<Chunk<T>>> {
        const { channelId, streamId } = await factory(this.channelId);
        return new Observable(subscriber => {
            this.streams.push({ stream: streamId, subscriber });
            if (!this.channelId) {
                this.spawnReader(channelId);
            }
            return () => { this.streams = this.streams.filter(({ stream }) => stream !== streamId) }
        });
    }
}

const poller = new MegaphonePoller('http://localhost:5173');
const o = await poller.newStream<{ message: string, sender: string }>(async channel => {
    let res = await fetch("http://localhost:5173/room/test", {
        method: "POST",
        headers: {
            ...(channel ? { 'use-channel': channel } : {}),
        },
    }).then(res => res.json());

    return {
        channelId: res.channelUuid,
        streamId: 'new-message',
    }
});

const chunk = await firstValueFrom(o);
console.log(chunk.body.message);