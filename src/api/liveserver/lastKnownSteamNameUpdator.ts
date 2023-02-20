import axios from 'axios';
import { SqlQueryBuilder } from '../../utils/SqlQueryBuilder';
import { liveServerDatabaseUtils } from '../../utils/liveServerDatabaseUtils';

export const lastKnownSteamNameUpdator = {
    UPDATE_STEAM_NAMES: () => {
        const getSteamIds = new Promise<{ SteamID: string }[]>(function (
            resolve,
            reject
        ) {
            const sqlQuery = new SqlQueryBuilder()
                .select('SteamID')
                .from('l4d2_stats_kether')
                .build();
            liveServerDatabaseUtils.performQuery(sqlQuery, (err, results) => {
                if (err) {
                    reject(err);
                }
                if (results) {
                    resolve(results);
                }
            });
        });

        function replaceInvalidCharacters(input: string): string {
            const regex =
                /[\[\]\^\/\\\{\}\(\)\*\+\?\.\<\>\|\-\!\@\#\$\%\&\=\\"']/g;
            const replacement = '';
            return input.replace(regex, replacement);
        }

        let groupedSteamIds: string[][] = [];
        let steamIDs: string[] = [];
        const group_size = 90;
        getSteamIds
            .then((rawStats) => {
                rawStats.forEach((rawStat) => {
                    steamIDs.push(rawStat.SteamID);
                });
            })
            .then(() => {
                for (
                    let group = 1;
                    group <= Math.ceil(steamIDs.length / group_size);
                    group++
                ) {
                    const groupContent = steamIDs.slice(
                        (group - 1) * group_size + 1,
                        group * group_size
                    );
                    groupedSteamIds.push(groupContent);
                }
            })
            .then(() => {
                groupedSteamIds.forEach((group, index) => {
                    setTimeout(() => {
                        const fetchURL = `https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v0002/?key=F9B6127DDEB6AF27EA0D64F1E5C642A4&steamids=${group.join(
                            ','
                        )}`;
                        axios
                            .request({
                                url: fetchURL,
                                method: 'get',
                            })
                            .then((response) => {
                                const playersInGroupData =
                                    response.data.response.players;
                                playersInGroupData.forEach(
                                    (player: {
                                        steamid: string;
                                        personaname: string;
                                        profileurl: string;
                                        avatarmedium: string;
                                    }) => {
                                        const sqlQuery = new SqlQueryBuilder()
                                            .update('l4d2_stats_kether')
                                            .set([
                                                {
                                                    columnName:
                                                        'LastKnownSteamName',
                                                    columnValue:
                                                        replaceInvalidCharacters(
                                                            player.personaname
                                                        ),
                                                },
                                                {
                                                    columnName: 'profileUrl',
                                                    columnValue:
                                                        player.profileurl,
                                                },
                                                {
                                                    columnName:
                                                        'avatarMediumSrc',
                                                    columnValue:
                                                        player.avatarmedium,
                                                },
                                            ])
                                            .whereColumnName('SteamID')
                                            .equals(player.steamid)
                                            .build();

                                        liveServerDatabaseUtils.performQuery(
                                            sqlQuery
                                        );
                                    }
                                );
                            })
                            .catch((error) => {
                                console.log(error);
                            });
                    }, 1000 * 60 * 5 * (index - 1));
                });
            });
    },
};
