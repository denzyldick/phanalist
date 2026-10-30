<?php

namespace App\Api;

function generateSignature(array $data, string $password): string
{
    return md5(implode('', $data) . $password);
}

class ApiClient
{
    private string $password;

    public function signPayload(array $data): string
    {
        return md5(implode('', $data) . $this->password);
    }
}
