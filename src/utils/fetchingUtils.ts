import axios from 'axios';

export const fetchingUtils = {
    fetchWrapHandleErrors: (
        request: string,
        options: {
            method: 'post' | 'get' | 'update';
            headers?: any;
            body?: URLSearchParams;
        }
    ) => {
        return axios
            .request({
                url: request,
                method: options.method,
                data: options.body,
                headers: options.headers,
            })
            .catch((error) => {
                console.log(
                    `Failed fetching: ${JSON.stringify(
                        request
                    )} with options ${JSON.stringify(options)}`
                );
                console.log(`Error: ${JSON.stringify(error)}`);
            })
            .then((response: any) => {
                return response?.data || false;
            });
    },
};
