import { Response } from 'express';
import { HttpStatusCode } from 'axios';

export const databaseUtils = {
    wrapDatabaseTaskRequest: (task: Promise<any>, res: Response) => {
        task.then((dbResponse) => {
            res.send(dbResponse);
        }).catch((error) => {
            res.status(HttpStatusCode.Forbidden).send(error.message).end();
        });
    },
};
