import axios from 'axios';
import { bindSuggestionsDatabaseUtils } from '../bindSuggestions/bindSuggestionsDatabaseUtils';
import { Database } from 'sqlite3';
import { SqlQueryBuilder, SelectorType } from '../../utils/SqlQueryBuilder';
import { liveServerDatabaseUtils } from '../../utils/liveServerDatabaseUtils';

export const bindSuggestionsLiveserverFetcher = {
    FETCH_PROCESS_AND_PUT_IN_OUR_DATABASE: (database: Database) => {
        const getLiveServerBindSuggestions = new Promise<
            { LP: number; SteamID: number; Content: string }[]
        >(function (resolve, reject) {
            const sqlQuery = new SqlQueryBuilder()
                .select(SelectorType.ALL)
                .from('l4d2_binds_kether')
                .build();
            liveServerDatabaseUtils.performQuery(sqlQuery, (error, results) => {
                error && reject(error);
                resolve(results);
            });
        });

        getLiveServerBindSuggestions.then((rawBindSuggestions) => {
            rawBindSuggestions.forEach((rawBindSuggestion) => {
                const sqlQuery = new SqlQueryBuilder()
                    .deleteFrom('l4d2_binds_kether')
                    .whereColumnName('LP')
                    .equals(String(rawBindSuggestion.LP))
                    .build();
                liveServerDatabaseUtils.performQuery(sqlQuery);
                // liveServerDatabaseUtils.dbConnection().query(
                //     sqlQuery
                //     // `DELETE FROM l4d2_binds_kether WHERE LP=${rawBindSuggestion.LP}`
                // );
                const splittedBind: string[] =
                    rawBindSuggestion.Content.split(':');
                const fetchURL = `https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v0002/?key=F9B6127DDEB6AF27EA0D64F1E5C642A4&steamids=${rawBindSuggestion.SteamID}`;

                axios
                    .request({ method: 'get', url: fetchURL })
                    .then((response) => {
                        const proposedBy =
                            response.data.response.players[0].personaname;
                        let author: string;
                        let text: string;
                        if (splittedBind.length === 2) {
                            author = splittedBind[0];
                            text = splittedBind[1];
                        } else {
                            author = 'Unknown';
                            text = rawBindSuggestion.Content;
                        }
                        bindSuggestionsDatabaseUtils.addBindSuggestion(
                            database,
                            {
                                author: author,
                                text: text,
                                proposedBy: proposedBy,
                            }
                        );
                    });
            });
        });
    },
};
