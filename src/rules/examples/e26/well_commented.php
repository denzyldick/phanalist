<?php

namespace App;

/**
 * Processes payments through the gateway.
 */
class WellCommented
{
    private $gateway;

    public function __construct($gateway)
    {
        $this->gateway = $gateway;
    }

    /**
     * Charge a customer for the given amount.
     */
    public function charge(float $amount): bool
    {
        if ($amount <= 0) {
            return false;
        }

        $result = $this->gateway->charge($amount);

        return $result->isSuccessful();
    }
}
