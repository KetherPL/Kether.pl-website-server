import { NextFunction, Request, Response } from 'express';

export const loggerUtils = {
    logRequests: (req: Request, res: Response, next: NextFunction) => {
        console.log(`\x1b[33mProcessing request: ${req.originalUrl} \x1b[0m`);
        next();
    },
};
