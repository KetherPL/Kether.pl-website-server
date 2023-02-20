import express, { RequestHandler } from 'express';
import cors from 'cors';
import {
    getBindSuggestionsDatabase,
    getBindsDatabase,
    getCommandsDatabase,
} from './database/databases';
import { bindsApi } from './api/binds/bindsApi';
import { bindSuggestionsApi } from './api/bindSuggestions/bindSuggestionsApi';
import { steamApi } from './api/steam/steamApi';
import { liveserverApi } from './api/liveserver/liveserverApi';
import { bindSuggestionsLiveserverFetcher } from './api/liveserver/bindSuggestionsLiveserverFetcher';
import { corsUtils } from './cors/corsUtils';
import { loggerUtils } from './utils/loggerUtils';
import { constUtils } from './utils/constUtils';
import { lastKnownSteamNameUpdator } from './api/liveserver/lastKnownSteamNameUpdator';
import { steamBot } from './steam/steamBot';
import { callForSubsLiveserverFetcher } from './api/liveserver/callForSubsLiveserverFetcher';
import { commandsApi } from './api/commands/commandsApi';
import { totalScoreUpdator } from './api/liveserver/totalScoreUpdator';

const app = express();
try {
    steamBot.login();
} catch (error) {
    console.log(
        "Couldn't login to steam bot account, reason: " + JSON.stringify(error)
    );
}

const databasesCollection = {
    bindsDatabase: getBindsDatabase(),
    bindSuggestionsDatabase: getBindSuggestionsDatabase(),
    commandsDatabse: getCommandsDatabase(),
};

app.use(express.urlencoded({ extended: true }) as RequestHandler);
app.use(express.json() as RequestHandler);
app.use(cors(corsUtils.CORS_OPTIONS));
app.use(corsUtils.setDefaultHeaders);
app.use(loggerUtils.logRequests);

bindsApi(app, databasesCollection.bindsDatabase);
bindSuggestionsApi(app, databasesCollection.bindSuggestionsDatabase);
commandsApi(app, databasesCollection.commandsDatabse);
steamApi(app);
liveserverApi(app);

setInterval(() => {
    bindSuggestionsLiveserverFetcher.FETCH_PROCESS_AND_PUT_IN_OUR_DATABASE(
        databasesCollection.bindSuggestionsDatabase
    );
}, 1 * 60 * 60 * 1000);

setInterval(() => {
    lastKnownSteamNameUpdator.UPDATE_STEAM_NAMES();
}, 1000 * 60 * 30);

setInterval(() => {
    callForSubsLiveserverFetcher.FETCH_PROCESS_AND_POST_ON_CHAT_GROUP();
}, 5000);

//On server reload/rerun, update scores, then update periodically
totalScoreUpdator.UPDATE_TOTAL_SCORE();
setTimeout(() => {
    totalScoreUpdator.UPDATE_PLACE_IN_RANKING();
}, 1000 * 10);
setInterval(() => {
    totalScoreUpdator.UPDATE_TOTAL_SCORE();
    setTimeout(() => {
        totalScoreUpdator.UPDATE_PLACE_IN_RANKING();
    }, 1000 * 10);
}, 1000 * 60 * 5);

//On server reload/rerun, then update periodically
totalScoreUpdator.AVERAGE_RECORDS_REDUCER(50);
setInterval(() => {
    totalScoreUpdator.AVERAGE_RECORDS_REDUCER(50);
}, 1000 * 60 * 24);

app.listen(constUtils.SERVER_PORT, () => {
    console.log(`Server listening at port ${constUtils.SERVER_PORT}`);
});
