import { SelectorType, SqlQueryBuilder } from '../../utils/SqlQueryBuilder';
import { liveServerDatabaseUtils } from '../../utils/liveServerDatabaseUtils';
import {
    AverageGameStatEntry,
    GameStatEntry,
} from '../../models/GameStatsModels';
import { liveserverApi, performOperationsOnGameStats } from './liveserverApi';
import { InformationStorage } from '../../storage/InformationStorage';

const getSteamIDsForTable = async (tableName: string) => {
    const sqlQuery = new SqlQueryBuilder()
        .selectDistinct(`SteamID`)
        .from(tableName)
        .build();

    const queryGet = new Promise<{ SteamID: string }[]>(function (
        resolve,
        reject
    ) {
        liveServerDatabaseUtils.performQuery(sqlQuery, (error, results) => {
            error && reject(error);
            resolve(results);
        });
    });

    let results: undefined | { SteamID: string }[] = undefined;
    try {
        results = await queryGet;
    } catch (error) {
        console.log(error);
    }

    return results;
};

const getRecordsForSteamID = async (SteamID: string, tableName: string) => {
    const sqlQueryEntries = new SqlQueryBuilder()
        .select(SelectorType.ALL)
        .from(tableName)
        .whereColumnName(`SteamID`)
        .equals(SteamID)
        .build();

    const queryRecordsOfPlayer = new Promise<AverageGameStatEntry[]>(function (
        resolve,
        reject
    ) {
        liveServerDatabaseUtils.performQuery(
            sqlQueryEntries,
            (error, results) => {
                error && reject(error);
                resolve(results);
            }
        );
    });

    let results: undefined | AverageGameStatEntry[] = undefined;
    try {
        results = await queryRecordsOfPlayer;
    } catch (error) {
        console.log(error);
    }

    return results;
};
//
// const getInsertQueriesForCommonsKilled = (
//     SteamID: string,
//     averagedStats: AverageGameStatEntry[],
//     tableName: string
// ) => {
//     let arrOfQueries: string[] = [];
//     for (const averageStat of averagedStats) {
//         arrOfQueries.push(
//             `INSERT INTO ${tableName} SET SteamID = ${SteamID}, Commons_Killed_In_Round_Entry = ${averageStat.Commons_Killed_In_Round_Entry} `
//         );
//     }
//     return arrOfQueries;
// };
//
// const averageCommonsKilledRecord = async (maxRecords: number = 20) => {
//     const tableName = 'l4d2_stats_kether_commons_killed_averages';
//     const SteamIDsInTable: { SteamID: string }[] | undefined =
//         await getSteamIDsForTable(tableName);
//     if (SteamIDsInTable) {
//         for (const steamObject of SteamIDsInTable) {
//             const recordsOfPlayer: AverageGameStatEntry[] | undefined =
//                 await getRecordsForSteamID(steamObject.SteamID, tableName);
//             if (recordsOfPlayer) {
//                 if (recordsOfPlayer.length > maxRecords) {
//                     console.log(
//                         `Player ${steamObject.SteamID} has ${recordsOfPlayer.length} entries in table ${tableName}. Reducing....`
//                     );
//                     const allValuesOfPlayer: number[] = recordsOfPlayer.map(
//                         (record) => record.Commons_Killed_In_Round_Entry
//                     );
//                     const resultAverageStatsOfPlayer: AverageGameStatEntry[] =
//                         [];
//
//                     let currentGroupBeingReduced: number[] = [];
//
//                     for (const value of allValuesOfPlayer) {
//                         currentGroupBeingReduced.push(value);
//                         if (currentGroupBeingReduced.length > 2) {
//                             let totalStatScore = 0;
//                             for (const partialScore of currentGroupBeingReduced) {
//                                 totalStatScore += partialScore;
//                             }
//                             const averageStat: AverageGameStatEntry = {
//                                 SteamID: steamObject.SteamID,
//                                 Commons_Killed_In_Round_Entry:
//                                     totalStatScore /
//                                     currentGroupBeingReduced.length,
//                             } as AverageGameStatEntry;
//                             resultAverageStatsOfPlayer.push(averageStat);
//                             currentGroupBeingReduced = [];
//                         }
//                     }
//                     if (currentGroupBeingReduced.length > 0) {
//                         let totalStatScore = 0;
//                         for (const partialScore of currentGroupBeingReduced) {
//                             totalStatScore += partialScore;
//                         }
//                         const averageStat: AverageGameStatEntry = {
//                             SteamID: steamObject.SteamID,
//                             Commons_Killed_In_Round_Entry:
//                                 totalStatScore /
//                                 currentGroupBeingReduced.length,
//                         } as AverageGameStatEntry;
//                         resultAverageStatsOfPlayer.push(averageStat);
//                         currentGroupBeingReduced = [];
//                     }
//                     //we have averaged entries properly calculated, now we need to do sql transaction to
//                     //delete existing values and then push new values, averaged ones
//                     const insertQueries = getInsertQueriesForCommonsKilled(
//                         steamObject.SteamID,
//                         resultAverageStatsOfPlayer,
//                         tableName
//                     );
//                     const sqlQueryTransactionReducerArr: string[] = [];
//                     sqlQueryTransactionReducerArr.push('START TRANSACTION ');
//                     sqlQueryTransactionReducerArr.push(
//                         `DELETE FROM ${tableName} WHERE SteamID = ${steamObject.SteamID} `
//                     );
//                     insertQueries.forEach((queryLine) => {
//                         sqlQueryTransactionReducerArr.push(queryLine);
//                     });
//                     sqlQueryTransactionReducerArr.push('COMMIT');
//                     const sqlQueryTransactionReducer =
//                         sqlQueryTransactionReducerArr.join('\n');
//
//                     const queryTransactional = new Promise(function (
//                         resolve,
//                         reject
//                     ) {
//                         liveServerDatabaseUtils.performQuery(
//                             sqlQueryTransactionReducer,
//                             (error, results) => {
//                                 error && reject(error);
//                                 resolve(results);
//                             }
//                         );
//                     });
//                     try {
//                         const queryResult = await queryTransactional;
//                         console.log(`Entries reduced`);
//                     } catch (error) {
//                         console.log(error);
//                     }
//                 }
//             }
//         }
//     }
// };

export const totalScoreUpdator = {
    AVERAGE_RECORDS_REDUCER: async (maxRecords: number = 25) => {
        const tablesToCheckForDateValidity: string[] = [
            `l4d2_stats_kether_commons_killed_averages`,
            `l4d2_stats_kether_damage_done_to_si_averages`,
            `l4d2_stats_kether_friendly_fire_done_averages`,
            `l4d2_stats_kether_friendly_recover_averages`,
            `l4d2_stats_kether_hunter_skeets_averages`,
        ];
        for (const tableName of tablesToCheckForDateValidity) {
            const steamObjects = await getSteamIDsForTable(tableName);
            if (steamObjects) {
                for (const steamObject of steamObjects) {
                    const records = await getRecordsForSteamID(
                        steamObject.SteamID,
                        tableName
                    );
                    if (records) {
                        if (records.length > maxRecords) {
                            console.log(
                                `Player ${steamObject.SteamID} has ${records.length} records in ${tableName} while maximum is ${maxRecords}. Reducing....`
                            );
                            function sortByDate(
                                a: AverageGameStatEntry,
                                b: AverageGameStatEntry
                            ) {
                                return (
                                    a.DateAdded.getTime() -
                                    b.DateAdded.getTime()
                                );
                            }
                            records.sort(sortByDate);

                            const amountOfRecordsToDelete =
                                records.length - maxRecords;
                            console.log(
                                `Deleting ${amountOfRecordsToDelete} oldest records`
                            );
                            const idsOfRecordsToDelete: number[] = [];
                            for (const record of records) {
                                idsOfRecordsToDelete.push(record.id);
                                if (
                                    idsOfRecordsToDelete.length >=
                                    amountOfRecordsToDelete
                                ) {
                                    break;
                                }
                            }
                            const sqlQuery = new SqlQueryBuilder().deleteFrom(
                                tableName
                            );
                            for (const [
                                index,
                                idOfRecordToDelete,
                            ] of idsOfRecordsToDelete.entries()) {
                                if (index === 0) {
                                    sqlQuery
                                        .whereColumnName('id')
                                        .equals(`${idOfRecordToDelete}`);
                                } else {
                                    sqlQuery
                                        .columnNameWithoutWhere('id')
                                        .equals(`${idOfRecordToDelete}`);
                                }
                                const isLastObj =
                                    index === idsOfRecordsToDelete.length - 1;
                                if (!isLastObj) {
                                    sqlQuery.or();
                                }
                            }
                            const finalQuery = sqlQuery.build();
                            const finalQueryPerformer = new Promise(function (
                                resolve,
                                reject
                            ) {
                                liveServerDatabaseUtils.performQuery(
                                    finalQuery,
                                    (error, results) => {
                                        error && reject(error);
                                        resolve(results);
                                    }
                                );
                            });
                            try {
                                const queryResult = await finalQueryPerformer;
                                console.log(`Reducing finished successfully.`);
                            } catch (error) {
                                console.log(error);
                            }
                        }
                    }
                }
            }
        }
    },
    UPDATE_TOTAL_SCORE: async () => {
        const sqlQueryBuilder = new SqlQueryBuilder()
            .select(SelectorType.ALL)
            .from('l4d2_stats_kether');
        const query = sqlQueryBuilder.build();

        const queryGet = new Promise(function (resolve, reject) {
            liveServerDatabaseUtils.performQuery(query, (error, results) => {
                error && reject(error);
                resolve(results);
            });
        });

        try {
            const gameStats: GameStatEntry[] =
                (await queryGet) as unknown as GameStatEntry[];
            const modifiedGameStats = await performOperationsOnGameStats(
                gameStats
            );
            //do not update total score if table is too small
            if (modifiedGameStats.length < 100) {
                return;
            }
            modifiedGameStats.forEach((modifiedGameStat) => {
                let totalScore = 0;
                if (modifiedGameStat.Commons_Killed_Per_Round_Average) {
                    totalScore +=
                        modifiedGameStat.Commons_Killed_Per_Round_Average;
                }
                if (modifiedGameStat.Damage_Done_To_SI_Per_Round_Average) {
                    totalScore +=
                        modifiedGameStat.Damage_Done_To_SI_Per_Round_Average /
                        10;
                }
                if (modifiedGameStat.Hunters_Skeeted_Per_Round_Average) {
                    totalScore +=
                        modifiedGameStat.Hunters_Skeeted_Per_Round_Average *
                        100;
                }
                if (modifiedGameStat.Friendly_Fire_Done_Per_Round_Average) {
                    totalScore +=
                        -modifiedGameStat.Friendly_Fire_Done_Per_Round_Average;
                }
                if (modifiedGameStat.Friendly_Recovers_Per_Round_Average) {
                    totalScore +=
                        modifiedGameStat.Friendly_Recovers_Per_Round_Average *
                        50;
                }
                if (totalScore > 1) {
                    const querySet = new SqlQueryBuilder()
                        .update('l4d2_stats_kether')
                        .set([
                            {
                                columnName: 'Total_Score',
                                columnValue: totalScore.toFixed(2),
                            },
                        ])
                        .whereColumnName('SteamID')
                        .equals(modifiedGameStat.SteamID)
                        .build();
                    liveServerDatabaseUtils.performQuery(querySet);
                }
            });
        } catch (error) {
            console.error('Error during updating total score:');
            console.error(error);
        }
    },
    UPDATE_PLACE_IN_RANKING: async () => {
        console.log(`Updating Place in Rank`);
        const gameStatsQuery = new SqlQueryBuilder()
            .selectMultiple([`SteamID`, `Total_Score`])
            .from(`l4d2_stats_kether`)
            .build();
        const queryGetGameStats = new Promise<GameStatEntry[]>(function (
            resolve,
            reject
        ) {
            liveServerDatabaseUtils.performQuery(
                gameStatsQuery,
                (error, results) => {
                    error && reject(error);
                    resolve(results as unknown as GameStatEntry[]);
                }
            );
        });
        const gameStatEntries = await queryGetGameStats;
        if (gameStatEntries) {
            function sortByScore(a: GameStatEntry, b: GameStatEntry) {
                return b.Total_Score - a.Total_Score;
            }
            gameStatEntries.sort(sortByScore);
            let placeInRank = 1;
            const queries: string[] = [];
            for (const entry of gameStatEntries) {
                const query = new SqlQueryBuilder()
                    .update(`l4d2_stats_kether`)
                    .set([
                        {
                            columnName: 'Place_In_Rank',
                            columnValue: placeInRank,
                        },
                    ])
                    .whereColumnName('SteamID')
                    .equals(entry.SteamID)
                    .build();
                queries.push(query);
                placeInRank++;
            }
            queries.forEach((queryToPerform) => {
                liveServerDatabaseUtils.performQuery(queryToPerform);
            });
            console.log(`Place in rank updated`);
        }
    },
};
