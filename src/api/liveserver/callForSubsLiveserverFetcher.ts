import axios from 'axios';
import { SqlQueryBuilder, SelectorType } from '../../utils/SqlQueryBuilder';
import { steamBot } from '../../steam/steamBot';
import { liveServerDatabaseUtils } from '../../utils/liveServerDatabaseUtils';

export const callForSubsLiveserverFetcher = {
    FETCH_PROCESS_AND_POST_ON_CHAT_GROUP: () => {
        const getLiveServerCallForSubsDatabase = new Promise<
            { LP: number; SteamID: string }[]
        >(function (resolve, reject) {
            const sqlQuery = new SqlQueryBuilder()
                .select(SelectorType.ALL)
                .from('l4d2_call_for_sub_kether')
                .build();

            liveServerDatabaseUtils.performQuery(sqlQuery, (error, results) => {
                error && reject(error);
                resolve(results);
            });
            // const dbPool = liveServerDatabaseUtils.dbConnection();
            // dbPool.query(sqlQuery, (error, rows) => {
            //     error && reject(error);
            //     dbPool.end();
            //     resolve(rows);
            // });
        });

        //SteamID3 to AccountID (strip the string)
        function extractAccId(input: string): string {
            const regex = /\[(U:\d+:\d+)\]/;
            const match = input.match(regex);

            if (match && match.length === 2) {
                const accId = match[1].split(':')[2];
                return accId;
            }
            return '';
        }

        //Necessary var and module for SteamID manipulation, as well as later AccountID mentioning
        var SID = require('steamid');

        getLiveServerCallForSubsDatabase.then((callsForSub) => {
            if (callsForSub && callsForSub.length > 0) {
                callsForSub.forEach((callForSub) => {
                    const sqlQuery = new SqlQueryBuilder()
                        .deleteFrom('l4d2_call_for_sub_kether')
                        .whereColumnName('LP')
                        .equals(String(callForSub.LP))
                        .build();
                    liveServerDatabaseUtils.performQuery(sqlQuery);

                    var sid = new SID(`${callForSub.SteamID}`);
                    //convert SteamID64 to SteamID3 and then strip it in-place to get the necessary AccountID format
                    const accid = extractAccId(sid.getSteam3RenderedID());

                    const fetchURL = `https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v0002/?key= < Your Steam API Key > steamids=${callForSub.SteamID}`;

                    axios
                        .request({ method: 'get', url: fetchURL })
                        .then((response) => {
                            const callerName =
                                response.data.response.players[0].personaname;
                            try {
                                steamBot.sendMessage(
                                    `[mention=${accid}]@${callerName}[/mention] called for a sub, [mention=here]@online[/mention]`
                                );
                            } catch (error) {
                                console.log(
                                    "Couldn't call for sub, reason: " +
                                        JSON.stringify(error)
                                );
                            }
                        });
                });
            }
        });
    },
};
