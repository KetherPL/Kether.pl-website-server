import { Database } from 'sqlite3';
import { SelectorType, SqlQueryBuilder } from '../../utils/SqlQueryBuilder';
import { CommandData } from '../../models/commandModels';

export const commandsDatabaseUtils = {
    COMMANDS_DATABASE_REF: 'commands',
    addCommand: async (
        db: Database,
        commandToAdd: { command: string; description: string }
    ) => {
        return new Promise(function (resolve, reject) {
            !commandToAdd && reject('No command entry was given');
            !commandToAdd.command && reject('No command was given');
            !commandToAdd.description && reject('No description was given');
            const insertQuery = new SqlQueryBuilder()
                .insertIntoTableValues(
                    commandsDatabaseUtils.COMMANDS_DATABASE_REF,
                    [
                        { columnName: 'command', value: commandToAdd.command },
                        {
                            columnName: 'description',
                            value: commandToAdd.description,
                        },
                    ]
                )
                .build();
            try {
                db.run(insertQuery, function (err: any, rows: any) {
                    if (err) {
                        reject(err);
                    }
                    resolve({ rows });
                });
            } catch (error) {
                reject(error);
            }
        });
    },

    updateCommand: async (db: Database, commandUpdateData: CommandData) => {
        return new Promise(function (resolve, reject) {
            !commandUpdateData.command &&
                !commandUpdateData.description &&
                reject('No command data was given');
            !commandUpdateData.id && reject('No command id was given');
            try {
                findExistingCommand(db, commandUpdateData.id)
                    .then(() => {
                        commandUpdateData.command &&
                            db.exec(
                                new SqlQueryBuilder()
                                    .update(
                                        commandsDatabaseUtils.COMMANDS_DATABASE_REF
                                    )
                                    .set([
                                        {
                                            columnName: 'command',
                                            columnValue:
                                                commandUpdateData.command!,
                                        },
                                    ])
                                    .whereColumnName('id')
                                    .equals(String(commandUpdateData.id)!)
                                    .build()
                            );
                        commandUpdateData.description &&
                            db.exec(
                                new SqlQueryBuilder()
                                    .update(
                                        commandsDatabaseUtils.COMMANDS_DATABASE_REF
                                    )
                                    .set([
                                        {
                                            columnName: 'description',
                                            columnValue:
                                                commandUpdateData.description!,
                                        },
                                    ])
                                    .whereColumnName('id')
                                    .equals(String(commandUpdateData.id)!)
                                    .build()
                            );
                        resolve({ updatedCommandID: commandUpdateData.id });
                    })
                    .catch((error) => {
                        reject(error);
                    });
            } catch (error) {
                reject(error);
            }
        });
    },
    deleteCommand: async (db: Database, deleteCommandData: CommandData) => {
        return new Promise(function (resolve, reject) {
            !deleteCommandData &&
                reject('Couldnt delete command, no deletion data was given');
            !deleteCommandData.id && reject('No command id was given');
            findExistingCommand(db, deleteCommandData.id)
                .then((rows) => {
                    if ((rows as []).length > 0) {
                        db.exec(
                            new SqlQueryBuilder()
                                .deleteFrom(
                                    commandsDatabaseUtils.COMMANDS_DATABASE_REF
                                )
                                .whereColumnName('id')
                                .equals(String(deleteCommandData.id))
                                .build()
                        );
                        resolve({
                            message:
                                'Successfully deleted command with id: ' +
                                deleteCommandData.id,
                        });
                    } else {
                        reject({ message: "Didn't find that command" });
                    }
                })
                .catch((error) => {
                    reject(error);
                });
        });
    },
    getCommands: async (db: Database) => {
        return new Promise(function (resolve, reject) {
            db.all(
                new SqlQueryBuilder()
                    .select(SelectorType.ALL)
                    .from(commandsDatabaseUtils.COMMANDS_DATABASE_REF)
                    .build(),
                function (err: any, rows: []) {
                    if (err) {
                        reject(err);
                    } else {
                        resolve(rows);
                    }
                }
            );
        });
    },
};

const findExistingCommand = async (db: Database, id: number) => {
    return new Promise(function (resolve, reject) {
        db.all(
            new SqlQueryBuilder()
                .select(SelectorType.ALL)
                .from(commandsDatabaseUtils.COMMANDS_DATABASE_REF)
                .whereColumnName('id')
                .equals(String(id))
                .build(),
            function (error: Error | null, rows) {
                if (error) {
                    reject(error);
                } else {
                    resolve(rows);
                }
            }
        );
    });
};
