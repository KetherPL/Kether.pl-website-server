import { Database } from 'sqlite3';
import { apiConstants } from '../apiConstants';
import { databaseUtils } from '../../utils/databaseUtils';
import { Response } from 'express';
import { commandsDatabaseUtils } from './commandsDatabaseUtils';
import { CommandData, NewCommandData } from '../../models/commandModels';

export const commandsApi = (app: any, database: Database) => {
    GET: app.get(
        `${apiConstants.API_BASE_PATH}${apiConstants.COMMANDS_PATH}/getCommands`,
        async (req: Request, res: Response) => {
            databaseUtils.wrapDatabaseTaskRequest(
                commandsDatabaseUtils.getCommands(database),
                res
            );
        }
    );

    ADD: app.post(
        `${apiConstants.API_BASE_PATH}${apiConstants.COMMANDS_PATH}/addCommand`,
        async (req: Request, res: Response) => {
            const commandToAdd = req.body as unknown as NewCommandData;
            databaseUtils.wrapDatabaseTaskRequest(
                commandsDatabaseUtils.addCommand(database, commandToAdd),
                res
            );
        }
    );

    UPDATE: app.post(
        `${apiConstants.API_BASE_PATH}${apiConstants.COMMANDS_PATH}/updateCommand`,
        async (req: Request, res: Response) => {
            const updateData = req.body as unknown as CommandData;
            databaseUtils.wrapDatabaseTaskRequest(
                commandsDatabaseUtils.updateCommand(database, updateData),
                res
            );
        }
    );

    DELETE: app.post(
        `${apiConstants.API_BASE_PATH}${apiConstants.COMMANDS_PATH}/deleteCommand`,
        async (req: Request, res: Response) => {
            const deleteData = req.body as unknown as CommandData;
            databaseUtils.wrapDatabaseTaskRequest(
                commandsDatabaseUtils.deleteCommand(database, deleteData),
                res
            );
        }
    );
};
