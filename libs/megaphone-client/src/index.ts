#! /usr/bin/env -S deno run --allow-net

import { Observable, Subscriber } from "rxjs";

type Chunk<T> = {
    sid: string;
    eid: string;
    ts: string;
    body: T;
};

interface StreamSpec<T> {
    stream: string;
    subscriber: Subscriber<Chunk<T>>;
    finalizer: (msg: Chunk<T>) => boolean;
}

class MegaphonePoller {
    private channelId?: string;
    private streams: Array<StreamSpec<unknown>> = [];
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

                                    const continueStream = stream?.finalizer(msg);
                                    if (!continueStream) {
                                        this.streams = this.streams.filter(({ stream }) => stream !== msg.sid);
                                    }
                                });
                        }
                    });
            }
        } catch(error) {
            for (const stream of this.streams) {
                stream.subscriber.error(error);
                stream.subscriber.complete();
            }
            this.streams = [];
        } finally {
            this.channelId = undefined;
        }
    }

    async newStream<T>(
            factory: (channelId?: string) => Promise<{ channelId: string, streamIds: string[] }>,
            finalizer: (streamId: string, message: Chunk<T>) => boolean,
        ): Promise<Observable<Chunk<T>>> {
        const { channelId, streamIds } = await factory(this.channelId);
        return new Observable<Chunk<T>>(subscriber => {
            for (const streamId of streamIds) {
                const stream = { 
                    stream: streamId, 
                    subscriber, 
                    finalizer: (msg: unknown) => finalizer(streamId, msg as Chunk<T>) 
                };
                this.streams.push(stream);
            }
            if (!this.channelId) {
                this.spawnReader(channelId);
            }
            return () => { this.streams = this.streams.filter(({ stream }) => !streamIds.includes(stream)) }
        });
    }

    async newUnboundedStream<T>(
        factory: (channelId?: string) => Promise<{ channelId: string, streamIds: string[] }>,
    ): Promise<Observable<Chunk<T>>> {
        return await this.newStream(factory, () => true);
    }

    async newDelayedResponse<T>(
        factory: (channelId?: string) => Promise<{ channelId: string, streamIds: string[] }>,
    ): Promise<Observable<Chunk<T>>> {
        return await this.newStream(factory, () => false);
    }
}