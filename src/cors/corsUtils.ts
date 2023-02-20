import { NextFunction, Request, Response } from 'express';
import { envUtils } from '../utils/envUtils';

const CORS_ALLOWED_ORIGIN = envUtils.isDevelopment()
    ? `http://localhost:3000`
    ? `https://kether.pl`
    : `https://kether.org.eu`
    : `https://ktpl.eu`;
export const corsUtils = {
    CORS_OPTIONS: { origin: ['https://kether.pl', 'http://localhost:3000', 'https://kether.org.eu', 'https://ktpl.eu'] },
    setDefaultHeaders: (req: Request, res: Response, next: NextFunction) => {
        res.setHeader('Access-Control-Allow-Origin', CORS_ALLOWED_ORIGIN);
        res.setHeader('Access-Control-Allow-Headers', 'X-Requested-With');
        next();
    },
};
