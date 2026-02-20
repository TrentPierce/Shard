/**
 * Custom error classes for the Shard SDK.
 */

export class ShardError extends Error {
    constructor(message: string) {
        super(message)
        this.name = 'ShardError'
    }
}

export class ShardAPIError extends ShardError {
    public readonly statusCode: number | undefined
    public readonly responseBody: string | undefined

    constructor(message: string, statusCode?: number, responseBody?: string) {
        super(message)
        this.name = 'ShardAPIError'
        this.statusCode = statusCode
        this.responseBody = responseBody
    }
}

export class ShardTimeoutError extends ShardError {
    constructor(message: string) {
        super(message)
        this.name = 'ShardTimeoutError'
    }
}

export class ShardConnectionError extends ShardError {
    constructor(message: string) {
        super(message)
        this.name = 'ShardConnectionError'
    }
}

export class ShardAuthError extends ShardError {
    constructor(message: string = 'Invalid or missing API key') {
        super(message)
        this.name = 'ShardAuthError'
    }
}
