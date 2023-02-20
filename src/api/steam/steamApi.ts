import { Request, Response } from 'express';
import { fetchingUtils } from '../../utils/fetchingUtils';
import { InformationStorage } from '../../storage/InformationStorage';

export const steamApi = (app: any) => {
    app.options('/api/steam/userData', (req: Request, res: Response) => {
        res.setHeader('Content-Type', 'application/json');
    });

    app.post('/api/steam/userData', async (req: Request, res: Response) => {
        res.setHeader('Content-Type', 'application/json');

        const fetchURL = `https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v0002/?key= < your steam key > &steamids=${req.body.userID}`;

        fetchingUtils
            .fetchWrapHandleErrors(fetchURL, {
                method: 'get',
            })
            .then((response) => {
                res.send(response);
            });
    });

    app.get('/api/steam/serverInfo', async (req: Request, res: Response) => {
        InformationStorage.storage
            .getSteamServerInfo()
            .then((steamServerInfo) => {
                res.send(steamServerInfo);
            });
    });

    app.options('/api/steam/games', (req: Request, res: Response) => {
        res.setHeader('Content-Type', 'application/json');
    });

    app.post('/api/steam/games', async (req: Request, res: Response) => {
        res.setHeader('Content-Type', 'application/json');

        const fetchURL = `https://api.steampowered.com/IPlayerService/GetOwnedGames/v0001/?key= < your steam key > &steamid=${req.body.userID}&format=json&include_appinfo=true`;

        fetchingUtils
            .fetchWrapHandleErrors(fetchURL, {
                method: 'get',
            })
            .then((response) => {
                const games: [
                    { name: string; appid: number; playtime_forever: number }
                ] = response.response.games;

                let responseObj = { ownsLeft4Dead2: false };

                //TODO there is probably better way: to test: game.appid.550 !== undefined
                if (games && games?.length > 0) {
                    for (let game of games) {
                        //L4D2 appid = 550
                        if (game.appid === 550) {
                            responseObj.ownsLeft4Dead2 = true;
                            break;
                        }
                    }
                }
                res.send(responseObj);
            });
    });
};
