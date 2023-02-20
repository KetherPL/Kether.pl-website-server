import { Database } from 'sqlite3';
import { BindData, BindVote } from '../../utils/bindsModels';
import { SelectorType, SqlQueryBuilder } from '../../utils/SqlQueryBuilder';

export const bindsDatabaseUtils = {
    BINDS_DATABASE_REF: 'binds',
    BIND_VOTINGS_DATABASE_REF: 'bind_votings',
    addBind: async (
        db: Database,
        bindToAdd: { author: string; text: string }
    ) => {
        return new Promise(function (resolve, reject) {
            !bindToAdd && reject('No bind was given');
            !bindToAdd.author && reject('No author was given');
            !bindToAdd.text && reject('No text was given');
            const insertQuery = new SqlQueryBuilder()
                .insertIntoTableValues(bindsDatabaseUtils.BINDS_DATABASE_REF, [
                    { columnName: 'author', value: bindToAdd.author },
                    { columnName: 'text', value: bindToAdd.text },
                ])
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

    updateBind: async (db: Database, bindUpdateData: BindData) => {
        return new Promise(function (resolve, reject) {
            !bindUpdateData.text &&
                !bindUpdateData.author &&
                reject('No text nor author were given');
            !bindUpdateData.id && reject('No bind id was given');
            try {
                findExistingBind(db, bindUpdateData.id)
                    .then(() => {
                        bindUpdateData.author &&
                            db.exec(
                                new SqlQueryBuilder()
                                    .update(
                                        bindsDatabaseUtils.BINDS_DATABASE_REF
                                    )
                                    .set([
                                        {
                                            columnName: 'author',
                                            columnValue: bindUpdateData.author!,
                                        },
                                    ])
                                    .whereColumnName('id')
                                    .equals(String(bindUpdateData.id)!)
                                    .build()
                            );
                        bindUpdateData.text &&
                            db.exec(
                                new SqlQueryBuilder()
                                    .update(
                                        bindsDatabaseUtils.BINDS_DATABASE_REF
                                    )
                                    .set([
                                        {
                                            columnName: 'text',
                                            columnValue: bindUpdateData.text!,
                                        },
                                    ])
                                    .whereColumnName('id')
                                    .equals(String(bindUpdateData.id)!)
                                    .build()
                            );
                        resolve({ updatedBindID: bindUpdateData.id });
                    })
                    .catch((error) => {
                        reject(error);
                    });
            } catch (error) {
                reject(error);
            }
        });
    },
    deleteBind: async (db: Database, deleteBindData: BindData) => {
        return new Promise(function (resolve, reject) {
            !deleteBindData &&
                reject('Couldnt delete bind, no deletion data was given');
            !deleteBindData.id && reject('No bind id was given');
            findExistingBind(db, deleteBindData.id)
                .then((rows) => {
                    if ((rows as []).length > 0) {
                        db.exec(
                            new SqlQueryBuilder()
                                .deleteFrom(
                                    bindsDatabaseUtils.BINDS_DATABASE_REF
                                )
                                .whereColumnName('id')
                                .equals(String(deleteBindData.id))
                                .build()
                        );
                        bindsDatabaseUtils
                            .deleteVotesForBind(
                                db,
                                deleteBindData.id.toString()
                            )
                            .then(() => {
                                resolve({
                                    message:
                                        'Successfully deleted bind with id: ' +
                                        deleteBindData.id,
                                });
                            })
                            .catch((error) => {
                                reject(
                                    'Something went wrong during deletion of votes for bind'
                                );
                            });
                    } else {
                        reject({ message: "Didn't find that bind" });
                    }
                })
                .catch((error) => {
                    reject(error);
                });
        });
    },
    getBinds: async (db: Database) => {
        return new Promise(function (resolve, reject) {
            db.all(
                new SqlQueryBuilder()
                    .select(SelectorType.ALL)
                    .from(bindsDatabaseUtils.BINDS_DATABASE_REF)
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
    getUserVoteForGivenBind: async (db: Database, bindData: BindVote) => {
        return new Promise<BindVote>(function (resolve, reject) {
            !bindData.votedBindID && reject('No votingBindID was given!');
            !bindData.voterSteamID && reject('No voterSteamID was given!');
            const getQuery = new SqlQueryBuilder()
                .select(SelectorType.ALL)
                .from(bindsDatabaseUtils.BIND_VOTINGS_DATABASE_REF)
                .whereMultipleColumnsEquals([
                    {
                        columnName: 'voterSteamID',
                        value: bindData.voterSteamID!,
                    },
                    { columnName: 'votedBindID', value: bindData.votedBindID! },
                ])
                .build();
            db.get(getQuery, (error, bindVoting: BindVote) => {
                error && reject(error);
                resolve(bindVoting);
            });
        });
    },
    createNewBindVote: async (db: Database, bindVoteData: BindVote) => {
        return new Promise<string>(function (resolve, reject) {
            !bindVoteData.vote && reject('No vote was given!');
            !bindVoteData.votedBindID && reject('No votedBindID was given!');
            !bindVoteData.voterSteamID && reject('No voterSteamID was given!');
            const insertNewVoteQuery = new SqlQueryBuilder()
                .insertIntoTableValues(
                    bindsDatabaseUtils.BIND_VOTINGS_DATABASE_REF,
                    [
                        {
                            columnName: 'voterSteamID',
                            value: bindVoteData.voterSteamID!,
                        },
                        {
                            columnName: 'votedBindID',
                            value: bindVoteData.votedBindID!,
                        },
                        {
                            columnName: 'vote',
                            value: bindVoteData.vote!,
                        },
                    ]
                )
                .build();
            db.exec(insertNewVoteQuery, (error) => {
                error && reject(error);
                resolve('Successfully made new vote');
            });
        });
    },
    updateExistingBindVote: async (
        db: Database,
        voteID: string,
        vote: string
    ) => {
        return new Promise<string>(function (resolve, reject) {
            !voteID && reject('No voteID was given');
            !vote && reject('No vote was given');
            const updateQuery = new SqlQueryBuilder()
                .update(bindsDatabaseUtils.BIND_VOTINGS_DATABASE_REF)
                .set([{ columnName: 'vote', columnValue: vote }])
                .whereColumnName('id')
                .equals(voteID)
                .build();
            db.exec(updateQuery, (error) => {
                error && reject(error);
                resolve('Bind Vote was updated');
            });
        });
    },
    setVote: async (db: Database, bindData: BindVote) => {
        return new Promise(function (resolve, reject) {
            bindsDatabaseUtils
                .getUserVoteForGivenBind(db, bindData)
                .then((existingBindVote) => {
                    if (existingBindVote) {
                        bindsDatabaseUtils
                            .updateExistingBindVote(
                                db,
                                existingBindVote.id!.toString(),
                                bindData.vote!
                            )
                            .then((responseMessage) => {
                                resolve(responseMessage);
                            })
                            .catch((error) => {
                                reject(error);
                            });
                    } else {
                        bindsDatabaseUtils
                            .createNewBindVote(db, bindData)
                            .then((response) => {
                                resolve(response);
                            })
                            .catch((error) => {
                                reject(error);
                            });
                    }
                })
                .catch((error) => {
                    error && reject(error);
                });
        });
    },
    undoVote: async (db: Database, bindData: BindVote) => {
        return new Promise(function (resolve, reject) {
            !bindData.voterSteamID && reject('No voterSteamID was given!');
            !bindData.votedBindID && reject('No votedBindID was given!');
            bindsDatabaseUtils
                .getUserVoteForGivenBind(db, bindData)
                .then((existingBindVote) => {
                    !existingBindVote.id &&
                        reject('Something went wrong, report to admins');
                    const deleteQuery = new SqlQueryBuilder()
                        .deleteFrom(
                            bindsDatabaseUtils.BIND_VOTINGS_DATABASE_REF
                        )
                        .whereColumnName('id')
                        .equals(existingBindVote.id!.toString())
                        .build();
                    db.exec(deleteQuery, (error) => {
                        error && reject(error);
                        resolve('Successfully deleted vote');
                    });
                })
                .catch((error) => {
                    reject(error);
                });
        });
    },
    getVotes: async (db: Database) => {
        return new Promise(function (resolve, reject) {
            const getQuery = new SqlQueryBuilder()
                .select(SelectorType.ALL)
                .from(bindsDatabaseUtils.BIND_VOTINGS_DATABASE_REF)
                .build();
            db.all(getQuery, (error, rows: BindVote[]) => {
                error && reject(error);
                resolve(rows);
            });
        });
    },
    deleteVotesForBind: async (db: Database, bindID: string) => {
        return new Promise(function (resolve, reject) {
            const getQuery = new SqlQueryBuilder()
                .select(SelectorType.ALL)
                .from(bindsDatabaseUtils.BIND_VOTINGS_DATABASE_REF)
                .whereColumnName('votedBindID')
                .equals(bindID)
                .build();
            db.all(getQuery, (error, rows: BindVote[]) => {
                error && reject(error);
                rows.forEach((row: BindVote) => {
                    if (row.id) {
                        const deleteQuery = new SqlQueryBuilder()
                            .deleteFrom(
                                bindsDatabaseUtils.BIND_VOTINGS_DATABASE_REF
                            )
                            .whereColumnName('votedBindID')
                            .equals(row.id.toString())
                            .build();
                        db.exec(deleteQuery);
                    }
                });
                resolve(rows);
            });
        });
    },
};

const findExistingBind = async (db: Database, bindID: number) => {
    return new Promise(function (resolve, reject) {
        db.all(
            new SqlQueryBuilder()
                .select(SelectorType.ALL)
                .from(bindsDatabaseUtils.BINDS_DATABASE_REF)
                .whereColumnName('id')
                .equals(String(bindID))
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
