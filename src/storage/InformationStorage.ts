import { fetchingUtils } from '../utils/fetchingUtils';
import { LiveServerInfo } from '../models/serverInfoModels';
import { PlayerResponse, queryGameServerPlayer } from 'steam-server-query';
import { envUtils } from '../utils/envUtils';
import NodeCache from 'node-cache';
import { AverageGameStatEntry } from '../models/GameStatsModels';
import { SelectorType, SqlQueryBuilder } from '../utils/SqlQueryBuilder';
import { liveServerDatabaseUtils } from '../utils/liveServerDatabaseUtils';

const serversInfoCache = new NodeCache({ stdTTL: 5 });
const averageGameStatsInfoCache = new NodeCache({ stdTTL: 600 });
const liveServerInfoCacheKey = 'liveserver';
const steamServerInfoCacheKey = 'steam';
const averageFriendlyFireDoneKey = 'average_GameStats_FF';
const averageHunterSkeetsKey = 'average_GameStats_Hunter_Skeets';
const averageCommonsKilledKey = 'average_GameStats_Commons_Killed';
const averageDamageDoneToSIKey = 'average_GameStats_Damage_Done_To_SI';
const averageFriendlyRecoversKey = 'average_GameStats_Friendly_Recovers';

export const InformationStorage = {
    storage: {
        getLiveServerInfo: async (): Promise<LiveServerInfo | undefined> => {
            let liveServerInfo: LiveServerInfo = serversInfoCache.get(
                liveServerInfoCacheKey
            ) as LiveServerInfo;
            if (!liveServerInfo) {
                try {
                    const newLiveServerInfo = await fetchLiveServerInfo();
                    serversInfoCache.set(
                        liveServerInfoCacheKey,
                        newLiveServerInfo
                    );
                } catch (error) {
                    console.log(error);
                }
            }
            liveServerInfo = serversInfoCache.get(
                liveServerInfoCacheKey
            ) as LiveServerInfo;

            return Promise.resolve(liveServerInfo);
        },
        getSteamServerInfo: async (): Promise<PlayerResponse | undefined> => {
            let steamServerInfo: PlayerResponse = serversInfoCache.get(
                steamServerInfoCacheKey
            ) as PlayerResponse;
            if (!steamServerInfo) {
                try {
                    const newSteamServerInfoInfo = await fetchSteamServerInfo();
                    serversInfoCache.set(
                        steamServerInfoCacheKey,
                        newSteamServerInfoInfo
                    );
                } catch (error) {
                    console.log(error);
                }
            }
            steamServerInfo = serversInfoCache.get(
                steamServerInfoCacheKey
            ) as PlayerResponse;

            return Promise.resolve(steamServerInfo);
        },
        getAverageFriendlyFireInfo: async (): Promise<
            AverageGameStatEntry[]
        > => {
            let averageFriendlyFireInfo: AverageGameStatEntry[] =
                averageGameStatsInfoCache.get(
                    averageFriendlyFireDoneKey
                ) as unknown as AverageGameStatEntry[];
            if (!averageFriendlyFireInfo) {
                const averageGameStat = await getAverageFriendlyFireEntries();
                averageGameStatsInfoCache.set(
                    averageFriendlyFireDoneKey,
                    averageGameStat
                );
            }
            averageFriendlyFireInfo = averageGameStatsInfoCache.get(
                averageFriendlyFireDoneKey
            ) as unknown as AverageGameStatEntry[];
            return Promise.resolve(averageFriendlyFireInfo);
        },
        getAverageKilledCommonsInfo: async (): Promise<
            AverageGameStatEntry[]
        > => {
            let averageGameStatEntries: AverageGameStatEntry[] =
                averageGameStatsInfoCache.get(
                    averageCommonsKilledKey
                ) as unknown as AverageGameStatEntry[];
            if (!averageGameStatEntries) {
                const averageGameStat = await getAverageKilledCommonsEntries();
                averageGameStatsInfoCache.set(
                    averageCommonsKilledKey,
                    averageGameStat
                );
            }
            averageGameStatEntries = averageGameStatsInfoCache.get(
                averageCommonsKilledKey
            ) as unknown as AverageGameStatEntry[];
            return Promise.resolve(averageGameStatEntries);
        },
        getAverageHunterSkeetsInfo: async (): Promise<
            AverageGameStatEntry[]
        > => {
            let averageGameStatEntries: AverageGameStatEntry[] =
                averageGameStatsInfoCache.get(
                    averageHunterSkeetsKey
                ) as unknown as AverageGameStatEntry[];
            if (!averageGameStatEntries) {
                const averageGameStat = await getAverageHunterSkeetsEntries();
                averageGameStatsInfoCache.set(
                    averageHunterSkeetsKey,
                    averageGameStat
                );
            }
            averageGameStatEntries = averageGameStatsInfoCache.get(
                averageHunterSkeetsKey
            ) as unknown as AverageGameStatEntry[];
            return Promise.resolve(averageGameStatEntries);
        },
        getAverageDamageDoneToSIInfo: async (): Promise<
            AverageGameStatEntry[]
        > => {
            let averageGameStatEntries: AverageGameStatEntry[] =
                averageGameStatsInfoCache.get(
                    averageDamageDoneToSIKey
                ) as unknown as AverageGameStatEntry[];
            if (!averageGameStatEntries) {
                const averageGameStat = await getAverageDamageDoneToSIEntries();
                averageGameStatsInfoCache.set(
                    averageDamageDoneToSIKey,
                    averageGameStat
                );
            }
            averageGameStatEntries = averageGameStatsInfoCache.get(
                averageDamageDoneToSIKey
            ) as unknown as AverageGameStatEntry[];
            return Promise.resolve(averageGameStatEntries);
        },
        getAverageFriendlyRecoversInfo: async (): Promise<
            AverageGameStatEntry[]
        > => {
            let averageGameStatEntries: AverageGameStatEntry[] =
                averageGameStatsInfoCache.get(
                    averageFriendlyRecoversKey
                ) as unknown as AverageGameStatEntry[];
            if (!averageGameStatEntries) {
                const averageGameStat =
                    await getAverageFriendlyRecoversEntries();
                averageGameStatsInfoCache.set(
                    averageFriendlyRecoversKey,
                    averageGameStat
                );
            }
            averageGameStatEntries = averageGameStatsInfoCache.get(
                averageFriendlyRecoversKey
            ) as unknown as AverageGameStatEntry[];
            return Promise.resolve(averageGameStatEntries);
        },
    },
};

const fetchLiveServerInfo = (): Promise<LiveServerInfo> => {
    try {
        return fetchingUtils
            .fetchWrapHandleErrors(
                'https://rec.liveserver.pl/api?channel=get_server_info&return_method=json',
                {
                    method: 'post',
                    headers: {
                        'content-type': 'application/x-www-form-urlencoded',
                    },
                    body: new URLSearchParams({
                        client_id: '',
                        pin: '',
                        server_id: '',
                    }),
                }
            )
            .then((serverInfo: LiveServerInfo) => {
                return Promise.resolve(serverInfo);
            });
    } catch (error) {
        return Promise.reject(error);
    }
};

const fetchSteamServerInfo = (): Promise<PlayerResponse> => {
    if (!envUtils.isDevelopment()) {
        return queryGameServerPlayer('51.83.217.86:29800', 1, 20)
            .then((res) => {
                return Promise.resolve(res);
            })
            .catch((err) => {
                console.error(err);
                return Promise.reject(err);
            });
    } else {
        return Promise.reject(undefined);
    }
};

const getAverageFriendlyFireEntries = (): Promise<AverageGameStatEntry[]> => {
    const sqlQueryGetAverageFriendlyFire = new SqlQueryBuilder()
        .select(SelectorType.ALL)
        .from('l4d2_stats_kether_friendly_fire_done_averages')
        .build();

    const queryGetAverageFriendlyFire = new Promise(function (resolve, reject) {
        liveServerDatabaseUtils.performQuery(
            sqlQueryGetAverageFriendlyFire,
            (error, results) => {
                error && reject(error);
                resolve(results);
            }
        );
    });

    return queryGetAverageFriendlyFire
        .then((response) => {
            return response as unknown as AverageGameStatEntry[];
        })
        .catch((error) => {
            console.log(error);
            return [] as AverageGameStatEntry[];
        });
};

const getAverageKilledCommonsEntries = (): Promise<AverageGameStatEntry[]> => {
    const sqlQueryGetAverageFriendlyFire = new SqlQueryBuilder()
        .select(SelectorType.ALL)
        .from('l4d2_stats_kether_commons_killed_averages')
        .build();

    const queryGetAverageFriendlyFire = new Promise(function (resolve, reject) {
        liveServerDatabaseUtils.performQuery(
            sqlQueryGetAverageFriendlyFire,
            (error, results) => {
                error && reject(error);
                resolve(results);
            }
        );
    });

    return queryGetAverageFriendlyFire
        .then((response) => {
            return response as unknown as AverageGameStatEntry[];
        })
        .catch((error) => {
            console.log(error);
            return [] as AverageGameStatEntry[];
        });
};

const getAverageHunterSkeetsEntries = (): Promise<AverageGameStatEntry[]> => {
    const sqlQueryGetAverageFriendlyFire = new SqlQueryBuilder()
        .select(SelectorType.ALL)
        .from('l4d2_stats_kether_hunter_skeets_averages')
        .build();

    const queryGetAverageFriendlyFire = new Promise(function (resolve, reject) {
        liveServerDatabaseUtils.performQuery(
            sqlQueryGetAverageFriendlyFire,
            (error, results) => {
                error && reject(error);
                resolve(results);
            }
        );
    });

    return queryGetAverageFriendlyFire
        .then((response) => {
            return response as unknown as AverageGameStatEntry[];
        })
        .catch((error) => {
            console.log(error);
            return [] as AverageGameStatEntry[];
        });
};

const getAverageFriendlyRecoversEntries = (): Promise<
    AverageGameStatEntry[]
> => {
    const sqlQueryGetAverageFriendlyRecovers = new SqlQueryBuilder()
        .select(SelectorType.ALL)
        .from('l4d2_stats_kether_friendly_recover_averages')
        .build();

    const queryGetAverageFriendlyRecovers = new Promise(function (
        resolve,
        reject
    ) {
        liveServerDatabaseUtils.performQuery(
            sqlQueryGetAverageFriendlyRecovers,
            (error, results) => {
                error && reject(error);
                resolve(results);
            }
        );
    });

    return queryGetAverageFriendlyRecovers
        .then((response) => {
            return response as unknown as AverageGameStatEntry[];
        })
        .catch((error) => {
            console.log(error);
            return [] as AverageGameStatEntry[];
        });
};

const getAverageDamageDoneToSIEntries = (): Promise<AverageGameStatEntry[]> => {
    const sqlQueryGetAverageFriendlyFire = new SqlQueryBuilder()
        .select(SelectorType.ALL)
        .from('l4d2_stats_kether_damage_done_to_si_averages')
        .build();

    const queryGetAverageFriendlyFire = new Promise(function (resolve, reject) {
        liveServerDatabaseUtils.performQuery(
            sqlQueryGetAverageFriendlyFire,
            (error, results) => {
                error && reject(error);
                resolve(results);
            }
        );
    });

    return queryGetAverageFriendlyFire
        .then((response) => {
            return response as unknown as AverageGameStatEntry[];
        })
        .catch((error) => {
            console.log(error);
            return [] as AverageGameStatEntry[];
        });
};
