import { Request, Response } from 'express';
import { SelectorType, SqlQueryBuilder } from '../../utils/SqlQueryBuilder';
import { liveServerDatabaseUtils } from '../../utils/liveServerDatabaseUtils';
import { InformationStorage } from '../../storage/InformationStorage';
import {
    AverageGameStatEntry,
    GameStatEntry,
} from '../../models/GameStatsModels';

export const liveserverApi = (app: any) => {
    app.get(
        '/api/liveserver/gameStats/partial',
        async (req: Request, res: Response) => {
            const pageSize: number = Number(req.query.pageSize) || 10;
            const page: number = Number(req.query.page) || 0;
            let sortField: string = 'LastKnownSteamName';
            const sortOrder: number = req.query.sortOrder
                ? Number(req.query.sortOrder)
                : 0;
            const query: string | undefined = req.query.query
                ? String(req.query.query)
                : undefined;
            const sortFieldFromQuery = req.query.sortField;
            if (
                sortFieldFromQuery &&
                String(sortFieldFromQuery) !== 'undefined'
            ) {
                sortField = String(sortFieldFromQuery);
            }

            let sqlQueryBuilder = new SqlQueryBuilder()
                .select(SelectorType.ALL)
                .from('l4d2_stats_kether');
            if (query) {
                sqlQueryBuilder
                    .whereColumnName('LastKnownSteamName')
                    .matches(query);
            }
            if (sortField) {
                sqlQueryBuilder.orderBy(
                    sortField,
                    sortOrder === 1 ? 'desc' : 'asc'
                );
            }
            sqlQueryBuilder.limit(pageSize).offset(page * pageSize);
            const sqlQuery = sqlQueryBuilder.build();

            const queryGet = new Promise(function (resolve, reject) {
                liveServerDatabaseUtils.performQuery(
                    sqlQuery,
                    (error, results) => {
                        error && reject(error);
                        resolve(results);
                    }
                );
            });

            try {
                const gameStats: GameStatEntry[] =
                    (await queryGet) as unknown as GameStatEntry[];
                const modifiedGameStats = await performOperationsOnGameStats(
                    gameStats
                );
                res.send(modifiedGameStats);
            } catch (error) {
                res.status(400).send(error);
            }
        }
    );

    app.get(
        '/api/liveserver/gameStats/totalRecords',
        async (req: Request, res: Response) => {
            const query: string | undefined = req.query.query
                ? String(req.query.query)
                : undefined;

            const dbTotalRecords: Promise<{ TOTAL_RECORDS: number }> =
                new Promise(function (resolve, reject) {
                    let sqlQueryBuilder = new SqlQueryBuilder()
                        .select(SelectorType.COUNT_ALL)
                        .as('TOTAL_RECORDS')
                        .from('l4d2_stats_kether');
                    if (query) {
                        sqlQueryBuilder
                            .whereColumnName('LastKnownSteamName')
                            .matches(query);
                    }
                    const sqlQuery = sqlQueryBuilder.build();
                    liveServerDatabaseUtils.performQuery(
                        sqlQuery,
                        (error, results) => {
                            error && reject(error);
                            resolve(results);
                        }
                    );
                });

            dbTotalRecords
                .then((totalCount) => {
                    res.send(totalCount);
                })
                .catch((error) => res.status(400).send(error));
        }
    );

    app.get(
        '/api/liveserver/serverInfo',
        async (req: Request, res: Response) => {
            const liveServerInfo =
                await InformationStorage.storage.getLiveServerInfo();
            res.send(liveServerInfo);
        }
    );
};

export const performOperationsOnGameStats = async (
    gameStats: GameStatEntry[]
) => {
    try {
        const averageFriendlyFireEntries =
            await InformationStorage.storage.getAverageFriendlyFireInfo();
        const averageHunterSkeetsEntries =
            await InformationStorage.storage.getAverageHunterSkeetsInfo();
        const averageCommonsKilledEntries =
            await InformationStorage.storage.getAverageKilledCommonsInfo();
        const averageDamageDoneToSIEntries =
            await InformationStorage.storage.getAverageDamageDoneToSIInfo();
        const averageFriendlyRecoversEntries =
            await InformationStorage.storage.getAverageFriendlyRecoversInfo();

        gameStats.forEach((gameStat) => {
            const playerSteamID = gameStat.SteamID;

            //friendly fire
            let friendlyFireDoneTotal = 0;
            const filteredFriendlyFireEntries =
                averageFriendlyFireEntries.filter(
                    (entry) => entry.SteamID === playerSteamID
                );
            filteredFriendlyFireEntries.forEach((entry) => {
                friendlyFireDoneTotal +=
                    entry.Friendly_Fire_Done_In_Round_Entry;
            });
            let divider = filteredFriendlyFireEntries.length;
            gameStat.Friendly_Fire_Done_Per_Round_Average =
                friendlyFireDoneTotal / divider;

            //hunter skeets
            let hunterSkeetsTotal = 0;
            const filteredHunterSkeetsEntries =
                averageHunterSkeetsEntries.filter(
                    (entry) => entry.SteamID === playerSteamID
                );
            filteredHunterSkeetsEntries.forEach((entry) => {
                hunterSkeetsTotal += entry.Hunters_Skeeted_In_Round_Entry;
            });
            divider = filteredHunterSkeetsEntries.length;
            gameStat.Hunters_Skeeted_Per_Round_Average =
                hunterSkeetsTotal / divider;
            //commons kills
            let commonsKilledTotal = 0;
            const filteredCommonsKilledEntries =
                averageCommonsKilledEntries.filter(
                    (entry) => entry.SteamID === playerSteamID
                );
            filteredCommonsKilledEntries.forEach((entry) => {
                commonsKilledTotal += entry.Commons_Killed_In_Round_Entry;
            });
            divider = filteredCommonsKilledEntries.length;
            gameStat.Commons_Killed_Per_Round_Average =
                commonsKilledTotal / divider;
            //damage done to si
            let damageDoneToSITotal = 0;
            const filteredDamageDoneToSIEntries =
                averageDamageDoneToSIEntries.filter(
                    (entry) => entry.SteamID === playerSteamID
                );
            filteredDamageDoneToSIEntries.forEach((entry) => {
                damageDoneToSITotal += entry.Damage_Done_To_SI_In_Round_Entry;
            });
            divider = filteredDamageDoneToSIEntries.length;
            gameStat.Damage_Done_To_SI_Per_Round_Average =
                damageDoneToSITotal / divider;

            //friendly recovers
            let friendlyRecoversTotal = 0;
            const filteredFriendlyRecoversEntries =
                averageFriendlyRecoversEntries.filter(
                    (entry) => entry.SteamID === playerSteamID
                );
            filteredFriendlyRecoversEntries.forEach((entry) => {
                friendlyRecoversTotal += entry.Friendly_Recover_In_Round_Entry;
            });
            divider = filteredFriendlyRecoversEntries.length;
            gameStat.Friendly_Recovers_Per_Round_Average =
                friendlyRecoversTotal / divider;
        });
    } catch (error) {
        console.log(error);
    }
    return Promise.resolve(gameStats);
};
