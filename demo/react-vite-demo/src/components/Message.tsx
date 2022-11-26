export interface MessageArgs {
    direction: 'sent' | 'received',
    message: string,
    ts: string,
}

export function Message({ direction, message, ts }: MessageArgs) {
    if (direction === 'received') {
        return <div className="flex w-full mt-2 space-x-3 max-w-xs">
            <div className="flex-shrink-0 h-10 w-10 rounded-full bg-gray-300"></div>
            <div>
                <div className="bg-gray-300 p-3 rounded-r-lg rounded-bl-lg">
                    <p className="text-sm">{message}</p>
                </div>
                <span className="text-xs text-gray-500 leading-none">{ts}</span>
            </div>
        </div>;
    } else {
        return <div className="flex w-full mt-2 space-x-3 max-w-xs ml-auto justify-end">
            <div>
                <div className="bg-blue-600 text-white p-3 rounded-l-lg rounded-br-lg">
                    <p className="text-sm">{message}</p>
                </div>
                <span className="text-xs text-gray-500 leading-none">{ts}</span>
            </div>
            <div className="flex-shrink-0 h-10 w-10 rounded-full bg-gray-300"></div>
        </div>;
    }
}