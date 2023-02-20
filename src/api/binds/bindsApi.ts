import { Database } from 'sqlite3';
import { apiConstants } from '../apiConstants';
import { databaseUtils } from '../../utils/databaseUtils';
import { Response } from 'express';
import { bindsDatabaseUtils } from './bindsDatabaseUtils';
import { BindData, BindVote, NewBindData } from '../../utils/bindsModels';

export const bindsApi = (app: any, database: Database) => {
    GET: app.get(
        `${apiConstants.API_BASE_PATH}${apiConstants.BINDS_BASE_PATH}/getBinds`,
        async (req: Request, res: Response) => {
            databaseUtils.wrapDatabaseTaskRequest(
                bindsDatabaseUtils.getBinds(database),
                res
            );
        }
    );

    ADD: app.post(
        `${apiConstants.API_BASE_PATH}${apiConstants.BINDS_BASE_PATH}/addBind`,
        async (req: Request, res: Response) => {
            const bindToAdd = req.body as unknown as NewBindData;
            console.log(bindToAdd);
            databaseUtils.wrapDatabaseTaskRequest(
                bindsDatabaseUtils.addBind(database, bindToAdd),
                res
            );
        }
    );

    UPDATE: app.post(
        `${apiConstants.API_BASE_PATH}${apiConstants.BINDS_BASE_PATH}/updateBind`,
        async (req: Request, res: Response) => {
            const updateData = req.body as unknown as BindData;
            databaseUtils.wrapDatabaseTaskRequest(
                bindsDatabaseUtils.updateBind(database, updateData),
                res
            );
        }
    );

    DELETE: app.post(
        `${apiConstants.API_BASE_PATH}${apiConstants.BINDS_BASE_PATH}/deleteBind`,
        async (req: Request, res: Response) => {
            const deleteBindData = req.body as unknown as BindData;
            databaseUtils.wrapDatabaseTaskRequest(
                bindsDatabaseUtils.deleteBind(database, deleteBindData),
                res
            );
        }
    );

    VOTE: app.post(
        `${apiConstants.API_BASE_PATH}${apiConstants.BINDS_BASE_PATH}/vote`,
        async (req: Request, res: Response) => {
            const bindVoteData = req.body as unknown as BindVote;
            databaseUtils.wrapDatabaseTaskRequest(
                bindsDatabaseUtils.setVote(database, bindVoteData),
                res
            );
        }
    );

    UNDO_VOTE: app.post(
        `${apiConstants.API_BASE_PATH}${apiConstants.BINDS_BASE_PATH}/vote/delete`,
        async (req: Request, res: Response) => {
            const bindVoteData = req.body as unknown as BindVote;
            databaseUtils.wrapDatabaseTaskRequest(
                bindsDatabaseUtils.undoVote(database, bindVoteData),
                res
            );
        }
    );

    GET_VOTES: app.get(
        `${apiConstants.API_BASE_PATH}${apiConstants.BINDS_BASE_PATH}/votes`,
        async (req: Request, res: Response) => {
            databaseUtils.wrapDatabaseTaskRequest(
                bindsDatabaseUtils.getVotes(database),
                res
            );
        }
    );
};
