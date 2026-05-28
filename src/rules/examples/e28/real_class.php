<?php

namespace App;

class PaymentProcessor
{
    private $gateway;

    public function __construct($gateway)
    {
        $this->gateway = $gateway;
    }

    public function process(float $amount): bool
    {
        $this->validate($amount);
        $charge = $this->gateway->charge($amount);
        $this->log($charge);

        return $charge->isSuccessful();
    }

    private function validate(float $amount): void
    {
        if ($amount <= 0) {
            throw new \InvalidArgumentException("Invalid amount");
        }
    }

    private function log($result): void
    {
        // do nothing
    }
}
